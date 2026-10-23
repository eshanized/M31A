//! Comprehensive Integration Test Suite for Project Genesis, Planning Governance,
//! Approval Gates, and Execution Integration.
//!
//! Enforces:
//! - Feature A: Project Discovery integration, fact persistence, brownfield indexing,
//!   ambiguity convergence, and decoupling convergence from human approval.
//! - Feature B: Runtime enforcement of configured human-approval gates
//!   (`require_charter_approval`, `require_architecture_approval`, `require_roadmap_approval`).
//! - Feature C: CLI / routing semantics consistency (`--plan` fails closed without model,
//!   `--no-plan` skips genesis planning).
//! - Feature D: End-to-end traceability preserving roadmap phase IDs and requirement keys
//!   on candidate tasks in the execution plan.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

use async_trait::async_trait;
use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::kernel::seams::planner::{
    PlanRequest, PlanService, UpstreamPlanContext, UpstreamRoadmapPhase,
};
use m31a::planning::service::PlanServiceImpl;
use m31a::runtime::{AppRuntime, PlanningChoice};
use m31a::workflow::genesis::discovery::{DiscoveryPillar, DiscoverySession};
use m31a::workflow::genesis::environment::WorkspaceEnvironment;
use m31a::workflow::genesis::intake::{GenesisOptions, GenesisRequest};

// -----------------------------------------------------------------------------
// Test Helpers
// -----------------------------------------------------------------------------

fn setup_empty_git_workspace(dir: &Path) {
    let output = Command::new("git")
        .args(["init", dir.to_str().unwrap()])
        .output()
        .expect("git init failed");
    assert!(output.status.success());

    let _ = Command::new("git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "config",
            "user.name",
            "Test User",
        ])
        .output();
    let _ = Command::new("git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "config",
            "user.email",
            "test@m31a.local",
        ])
        .output();

    let gitignore = dir.join(".gitignore");
    std::fs::write(&gitignore, "target/\n.m31a/\n").expect("write .gitignore failed");

    let _ = Command::new("git")
        .args(["-C", dir.to_str().unwrap(), "add", ".gitignore"])
        .output();
    let _ = Command::new("git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "commit",
            "-m",
            "Initial commit",
        ])
        .output();
}

fn setup_brownfield_git_workspace(dir: &Path) {
    setup_empty_git_workspace(dir);

    let cargo_toml = r#"[package]
name = "sample-service"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = "1"
"#;
    std::fs::write(dir.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml failed");

    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("create src failed");
    std::fs::write(
        src_dir.join("lib.rs"),
        "pub fn process_data(input: &str) -> String { input.to_uppercase() }\n",
    )
    .expect("write lib.rs failed");

    let _ = Command::new("git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "add",
            "Cargo.toml",
            "src/lib.rs",
        ])
        .output();
    let _ = Command::new("git")
        .args([
            "-C",
            dir.to_str().unwrap(),
            "commit",
            "-m",
            "Add rust codebase",
        ])
        .output();
}

struct DummyTestModel {
    pub call_count: AtomicUsize,
    pub workspace_root: PathBuf,
}

impl DummyTestModel {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            workspace_root: workspace_root.into(),
        }
    }
}

#[async_trait]
impl ModelCaller for DummyTestModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let research_dir = self
            .workspace_root
            .join(".m31a")
            .join("project")
            .join("research");
        let _ = std::fs::create_dir_all(&research_dir);

        let files = [
            ("STACK.md", "# Tech Stack\nRust 2024, Tokio, SQLite\n"),
            ("FEATURES.md", "# Features\nCore engine features\n"),
            (
                "ARCHITECTURE.md",
                "# Architecture\nLayered subsystem architecture\n",
            ),
            ("PITFALLS.md", "# Pitfalls\nAvoid unbounded retry loops\n"),
            (
                "SECURITY.md",
                "# Security\nStrict capabilities and sandboxing\n",
            ),
            ("DEPLOYMENT.md", "# Deployment\nStandalone binary\n"),
            ("SUMMARY.md", "# Summary\nResearch completed\n"),
        ];

        for (name, content) in files {
            let _ = std::fs::write(research_dir.join(name), content);
        }

        Ok(ModelProposal::Complete {
            summary: "Completed research and planning synthesis".to_string(),
            artifacts: vec![],
        })
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[test]
fn test_feature_a_discovery_answers_propagate_to_charter_and_facts() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a distributed cache server in Rust", dir.path())
        .with_options(GenesisOptions::default().with_require_charter_approval(true));

    let mut session = DiscoverySession::new(req, env);
    assert_eq!(session.current_ambiguity, 95);
    assert!(!session.is_converged);

    while let Ok(Some(turn)) = session.next_turn() {
        let resp = match turn.pillar_focus {
            DiscoveryPillar::ProblemAndPersonas => {
                "Backend engineers building latency-critical microservices"
            }
            DiscoveryPillar::BoundariesAndNonGoals => {
                "In-memory LRU cache with RESP protocol; no persistent disk tier in v1"
            }
            DiscoveryPillar::TechnicalPreferences => {
                "Rust 2024 edition, tokio async I/O, parking_lot mutexes"
            }
            DiscoveryPillar::OperationalInvariants => {
                "MIT license, p99 latency under 200 microseconds"
            }
        };
        let converged = session.submit_response(resp).unwrap();
        if converged {
            break;
        }
    }

    assert!(session.is_converged);
    assert!(session.convergence_reason.is_some());

    // Synthesize charter
    let mut charter = session.synthesize_charter().unwrap();
    charter.validate().unwrap();

    // Verify user answers directly enriched the charter
    assert_eq!(charter.project_name, "Distributed Cache Server In Rust");
    assert!(
        charter
            .target_personas
            .iter()
            .any(|p| p.contains("Backend engineers"))
    );
    assert!(
        charter
            .boundaries
            .non_goals
            .iter()
            .any(|o| o.contains("persistent disk tier"))
    );

    // Verify truthful decoupling: convergence != human approval
    // Because require_charter_approval is true and convergence was automatic,
    // charter.confirmed_by_user must be false.
    assert!(!charter.confirmed_by_user);

    // Explicit human approval action
    charter.approve();
    assert!(charter.confirmed_by_user);
}

#[test]
fn test_feature_a_brownfield_discovery_indexes_code_and_reduces_ambiguity() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert!(env.is_brownfield());
    assert!(env.detected_stack.as_deref().unwrap().contains("Rust"));

    let req = GenesisRequest::new("Extend sample-service with metric export", dir.path());
    let session = DiscoverySession::new(req, env);

    // Brownfield discovery immediately registers existing repository facts
    assert!(
        session
            .facts
            .iter()
            .any(|f| f.key == "workspace_mode" && f.value == "brownfield")
    );
    assert!(
        session
            .facts
            .iter()
            .any(|f| f.key == "existing_stack" && f.value.contains("Rust"))
    );

    // Ambiguity must be reduced compared to greenfield baseline (95)
    assert!(session.current_ambiguity < 95);
}

#[tokio::test]
async fn test_feature_b_approval_gates_enforced_in_unattended_execution() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(DummyTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    // 1. With require_charter_approval = true and unattended execution (wait_for_approval = false),
    // running execution must fail closed with validation error.
    let charter_err = runtime
        .run_execution_with_choice(
            "Build a web service",
            None,
            false, // unattended: wait_for_approval = false
            None,
            Some(PlanningChoice::ProjectPlanning),
        )
        .await;

    assert!(charter_err.is_err());
    let err_msg = charter_err.unwrap_err().to_string();
    assert!(
        err_msg.contains("Project Charter human approval is required"),
        "Unexpected error: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_feature_c_explicit_plan_flag_without_model_fails_closed() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    // Create runtime without model caller
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed");

    // Explicit --plan without model caller must return a truthful error
    let res = runtime
        .run_execution_with_choice(
            "Build an authentication system",
            None,
            false,
            None,
            Some(PlanningChoice::ProjectPlanning),
        )
        .await;

    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg
            .contains("Project planning requested via --plan, but no model provider is configured"),
        "Expected missing model provider error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_feature_c_explicit_no_plan_flag_skips_genesis_planning() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(DummyTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    // With explicit --no-plan, genesis project planning is skipped
    // (even for a prompt that might otherwise trigger greenfield genesis).
    // Note: It reaches direct execution / plan generation without running Genesis.
    let res = runtime
        .run_execution_with_choice(
            "Build a fast compiler plugin",
            None,
            false,
            None,
            Some(PlanningChoice::DirectExecution),
        )
        .await;

    // Verify it didn't fail with "Project Charter human approval is required"
    // (which would only happen if Genesis project planning ran).
    if let Err(e) = res {
        let msg = e.to_string();
        assert!(
            !msg.contains("Project Charter human approval is required"),
            "Genesis should not have run under --no-plan: {}",
            msg
        );
    }
}

#[tokio::test]
async fn test_feature_d_roadmap_phases_traceability_to_candidate_tasks() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    // Build upstream context with 3 sequential roadmap phases and requirements
    let upstream = UpstreamPlanContext {
        project_name: "Auth Service".to_string(),
        charter: "# Charter\nSecure JWT Authentication Service\n".to_string(),
        requirements: vec![
            "REQ-01: JWT Token Generation".to_string(),
            "REQ-02: Argon2 Password Hashing".to_string(),
            "REQ-03: OAuth2 Integration".to_string(),
        ],
        roadmap_phases: vec![
            UpstreamRoadmapPhase {
                id: "PHASE-01".to_string(),
                name: "Core Password Hashing".to_string(),
                objective: "Implement Argon2 password hashing and verification".to_string(),
                dependencies: Vec::new(),
                requirement_refs: vec!["REQ-02".to_string()],
                architecture_refs: vec!["ARCH-SEC-01".to_string()],
                exit_criteria: vec!["Passwords hashed with Argon2id".to_string()],
                verification_strategy: "Unit test suite with password vectors".to_string(),
            },
            UpstreamRoadmapPhase {
                id: "PHASE-02".to_string(),
                name: "JWT Token Engine".to_string(),
                objective: "Implement JWT signing, claims validation, and refresh tokens".to_string(),
                dependencies: vec!["PHASE-01".to_string()],
                requirement_refs: vec!["REQ-01".to_string()],
                architecture_refs: vec!["ARCH-TOK-01".to_string()],
                exit_criteria: vec!["Tokens signed with RS256/EdDSA".to_string()],
                verification_strategy: "Integration tests for token issuance and expiry".to_string(),
            },
            UpstreamRoadmapPhase {
                id: "PHASE-03".to_string(),
                name: "OAuth2 Provider Integration".to_string(),
                objective: "Integrate Google and GitHub OAuth2 authorization flows".to_string(),
                dependencies: vec!["PHASE-02".to_string()],
                requirement_refs: vec!["REQ-03".to_string()],
                architecture_refs: vec!["ARCH-OAUTH-01".to_string()],
                exit_criteria: vec!["OAuth callback handling tested".to_string()],
                verification_strategy: "Mock OAuth provider end-to-end flow".to_string(),
            },
        ],
        ..Default::default()
    };

    // Instantiate PlanServiceImpl without a model caller (deterministic roadmap lowering)
    let planner = PlanServiceImpl::new(dir.path());
    let req = PlanRequest::new(
        m31a::ids::MissionId::from(uuid::Uuid::now_v7()),
        "Implement authentication service according to roadmap",
    )
    .with_upstream_context(upstream);

    let plan_resp = planner.generate_initial_plan(req).await.unwrap();
    let tasks = plan_resp.candidate_plan.tasks;

    assert_eq!(tasks.len(), 3);

    // Verify task 1 provenance
    let task1 = &tasks[0];
    assert_eq!(task1.id.as_str(), "PHASE-01/TASK-01");
    assert_eq!(task1.phase_id.as_deref(), Some("PHASE-01"));
    assert_eq!(task1.requirement_keys, vec!["REQ-02".to_string()]);
    assert!(task1.depends_on.is_empty());

    // Verify task 2 provenance and dependency
    let task2 = &tasks[1];
    assert_eq!(task2.id.as_str(), "PHASE-02/TASK-01");
    assert_eq!(task2.phase_id.as_deref(), Some("PHASE-02"));
    assert_eq!(task2.requirement_keys, vec!["REQ-01".to_string()]);
    assert_eq!(task2.depends_on.len(), 1);
    assert_eq!(task2.depends_on[0].as_str(), "PHASE-01/TASK-01");

    // Verify task 3 provenance and dependency
    let task3 = &tasks[2];
    assert_eq!(task3.id.as_str(), "PHASE-03/TASK-01");
    assert_eq!(task3.phase_id.as_deref(), Some("PHASE-03"));
    assert_eq!(task3.requirement_keys, vec!["REQ-03".to_string()]);
    assert_eq!(task3.depends_on.len(), 1);
    assert_eq!(task3.depends_on[0].as_str(), "PHASE-02/TASK-01");
}
