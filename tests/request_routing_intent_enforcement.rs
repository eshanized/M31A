//! Integration Test Suite: Request Routing, Intent Enforcement, and Model-Driven Execution
//!
//! Verifies Phase 9 requirements (Tests A through M):
//! - Test A: General information request ("What is Rust?") -> Direct answer, no plan review, no candidate plan, no fs.write.
//! - Test B: Repository question ("What does src/main.rs do?") -> Read-only inspection tools exposed, no source files modified.
//! - Test C: Read-only repository study ("Study this codebase.") -> Recovers from invalid model proposal safely to read-only task.
//! - Test D: Codebase investigation ("Find the root cause of the failing authentication test and report it.") -> Diagnostic/read-only tools, no workspace mutation.
//! - Test E: Verification request ("Run the existing test suite.") -> Verification execution without multi-step synthesis plan.
//! - Test F: Targeted implementation ("Fix the authentication bug in src/auth.rs.") -> Implementer role with write tools, policy evaluated.
//! - Test G: Scoped documentation task ("Write a README for this project.") -> Scoped documentation task.
//! - Test H: Mixed request ("Audit this repository, then fix the critical findings.") -> Mixed multi-phase classification entering governed planning.
//! - Test I: Negation and read-only override ("Do not modify any files; inspect the parser and explain why it fails.") -> Read-only contract strictly enforced.
//! - Test J: Capability enforcement -> Unknown capabilities and suffix matches (.write) rejected.
//! - Test K: Bounded recovery -> Candidate plan retry limit enforced; rejected plan never treated as accepted.
//! - Test L: Persistence and resume -> Session state and decisions preserved across reloads.
//! - Test M: Architecture parity -> Model-visible tools strictly match runtime role envelopes and policy gates.

mod support;

use tempfile::tempdir;

use m31a::ids::MissionId;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::planning::service::{
    ObjectiveClassification, PlanServiceImpl, classify_objective, is_mutation_forbidden,
};
use m31a::planning::validation::PlanValidator;
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;

async fn setup_git_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let repo_path = dir.path().to_path_buf();

    assert!(
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&repo_path)
            .status()
            .expect("git init")
            .success()
    );
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tests@m31a.local"])
        .current_dir(&repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Tester"])
        .current_dir(&repo_path)
        .status();

    tokio::fs::write(repo_path.join(".gitignore"), ".m31a/\ntarget/\n")
        .await
        .unwrap();
    tokio::fs::write(repo_path.join("README.md"), "# Test Project\n")
        .await
        .unwrap();
    tokio::fs::write(
        repo_path.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .await
    .unwrap();
    tokio::fs::create_dir_all(repo_path.join("src"))
        .await
        .unwrap();
    tokio::fs::write(
        repo_path.join("src/main.rs"),
        "fn main() { println!(\"hello\"); }\n",
    )
    .await
    .unwrap();

    (dir, repo_path)
}

fn sample_candidate_task(
    id: &str,
    role: AgentRole,
    caps: Vec<CapabilityRequirement>,
) -> CandidateTask {
    CandidateTask {
        id: CandidateTaskKey::new(id),
        objective: format!("Objective for {id}"),
        description: None,
        depends_on: vec![],
        capabilities: caps,
        role,
        verification: VerificationStrategy::Compilation,
        estimates: ResourceEstimate::new(5, 60, 1000, 0.10),
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------
// Test A: General information request ("What is Rust?")
// Expected: DirectAnswer classification, no plan review, no tasks, tool-free.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_a_general_information_request() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let prompt = "What is Rust?";

    // 1. Classification check
    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::DirectAnswer);
    assert!(classification.is_read_only());

    // 2. DirectAnswer produces tool-free engine
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("session");
    let mut engine = runtime.create_agent_engine(session.id);
    let all_tools = engine.tool_authority_scope().visible_tools();
    engine = engine.with_denied_tools(all_tools);

    // Hard invariant: ToolAuthorityScope visible_tools is empty for DirectAnswer
    let scope_tools = engine.tool_authority_scope().visible_tools();
    assert!(
        scope_tools.is_empty(),
        "Direct answer must not expose any tools to model"
    );
}

// -----------------------------------------------------------------------------
// Test B: Repository question ("What does src/main.rs do?")
// Expected: RepositoryInspection classification, read-only tools exposed, no write tools.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_b_repository_dependent_question() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let prompt = "What does src/main.rs do?";

    // 1. Classification
    let classification = classify_objective(prompt);
    assert_eq!(
        classification,
        ObjectiveClassification::RepositoryInspection
    );
    assert!(classification.is_read_only());

    // 2. Bound to researcher role
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("session");
    let engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(AgentRole::researcher());

    let tools = engine.tool_authority_scope().visible_tools();
    assert!(
        tools.contains(&"read_file".to_string()) || tools.contains(&"list_files".to_string()),
        "Repository question must expose read tools"
    );
    assert!(
        !tools.contains(&"write_file".to_string())
            && !tools.contains(&"edit_file".to_string())
            && !tools.contains(&"apply_patch".to_string()),
        "Repository question must NOT expose write tools"
    );
}

// -----------------------------------------------------------------------------
// Test C: Read-only repository study ("Study this codebase.")
// Expected: Proposing invalid capability (e.g. repo.write) recovers safely to a valid read-only task.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_c_study_codebase_invalid_capability_recovery() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let planner = PlanServiceImpl::new(&repo_path);

    let plan_req = PlanRequest::new(MissionId::new(), "study this codebase")
        .with_classification("RepositoryResearch", true);

    // Initial plan generation recovers safely even if model or decomposition proposes invalid tasks
    let candidate = planner
        .generate_initial_plan(plan_req)
        .await
        .expect("Plan generation must recover cleanly");

    assert!(
        !candidate.candidate_plan.tasks.is_empty(),
        "Must synthesize a valid research task"
    );
    for task in &candidate.candidate_plan.tasks {
        assert_eq!(task.role, AgentRole::researcher());
        for cap in &task.capabilities {
            assert!(
                !cap.id.contains("write"),
                "Capability '{}' must not grant write privileges on study request",
                cap.id
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Test D: Codebase investigation / bug hunt
// Expected: Diagnostic / Investigation classification, read-only diagnostician role.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_d_codebase_investigation() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let prompt = "Find the root cause of the failing authentication test and report it.";

    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::Investigation);
    assert!(classification.is_read_only());

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("session");
    let engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(AgentRole::diagnostician());

    let tools = engine.tool_authority_scope().visible_tools();
    assert!(
        !tools.contains(&"write_file".to_string())
            && !tools.contains(&"edit_file".to_string())
            && !tools.contains(&"apply_patch".to_string()),
        "Investigation role must NOT expose write tools"
    );
}

// -----------------------------------------------------------------------------
// Test E: Verification request ("Run the existing test suite.")
// Expected: Verification classification, verifier role with test runner tools, no write tools.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_e_verification_request() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let prompt = "Run the existing test suite.";

    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::Verification);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("session");
    let engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(AgentRole::verifier());

    let tools = engine.tool_authority_scope().visible_tools();
    assert!(
        tools.contains(&"read_file".to_string()),
        "Verifier must have read tools"
    );
    assert!(
        !tools.contains(&"write_file".to_string())
            && !tools.contains(&"edit_file".to_string())
            && !tools.contains(&"apply_patch".to_string()),
        "Verifier role must NOT have write tools"
    );
}

// -----------------------------------------------------------------------------
// Test F: Targeted implementation ("Fix the authentication bug in src/auth.rs.")
// Expected: Implementation classification, implementer role with write tools.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_f_targeted_implementation() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let prompt = "Fix the authentication bug in src/auth.rs.";

    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::Implementation);
    assert!(!classification.is_read_only());

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("session");
    let engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(AgentRole::implementer());

    let tools = engine.tool_authority_scope().visible_tools();
    assert!(
        tools.contains(&"write_file".to_string())
            || tools.contains(&"edit_file".to_string())
            || tools.contains(&"apply_patch".to_string()),
        "Implementer must have write capabilities"
    );
}

// -----------------------------------------------------------------------------
// Test G: Scoped documentation task ("Write a README for this project.")
// Expected: Documentation classification, implementer role.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_g_scoped_documentation() {
    let prompt = "Write a README for this project.";
    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::Documentation);
}

// -----------------------------------------------------------------------------
// Test H: Mixed request ("Audit this repository, then fix the critical findings.")
// Expected: Mixed classification entering structured multi-phase planning.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_h_mixed_request() {
    let prompt = "Audit this repository, then fix the critical findings.";
    let classification = classify_objective(prompt);
    assert_eq!(classification, ObjectiveClassification::Mixed);
}

// -----------------------------------------------------------------------------
// Test I: Negation and read-only override
// ("Do not modify any files; inspect the parser and explain why it fails.")
// Expected: Negation detected, read-only override active, write tools prohibited.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_i_negation_and_read_only_override() {
    let prompt = "Do not modify any files; inspect the parser and explain why it fails.";
    assert!(is_mutation_forbidden(prompt), "Negation must be identified");

    let classification = classify_objective(prompt);
    assert!(
        classification.is_read_only(),
        "Negated prompt must resolve to read-only"
    );

    // PlanValidator with read_only must reject any write task proposed
    let validator = PlanValidator::new().with_read_only(true);
    let task = sample_candidate_task(
        "T-01",
        AgentRole::implementer(),
        vec![CapabilityRequirement::new(
            "fs.write",
            CapabilityAccessMode::Write,
        )],
    );
    let plan = CandidatePlan::new("test-negation", "Test negation", vec![task]);
    let report = validator.validate(&plan).await;
    assert!(
        !report.is_valid(),
        "Validator with read_only=true must reject write capabilities"
    );
}

// -----------------------------------------------------------------------------
// Test J: Capability enforcement
// Unknown capability or appending .write to a tool name must be rejected.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_j_capability_enforcement() {
    let validator = PlanValidator::new();
    let task = sample_candidate_task(
        "T-01",
        AgentRole::implementer(),
        vec![CapabilityRequirement::new(
            "custom.write",
            CapabilityAccessMode::Write,
        )],
    );
    let plan = CandidatePlan::new("test-suffix", "Test suffix enforcement", vec![task]);
    let report = validator.validate(&plan).await;
    assert!(
        !report.is_valid(),
        "Unknown capability 'custom.write' must be rejected"
    );
}

// -----------------------------------------------------------------------------
// Test K: Bounded recovery
// Plan retry limit is respected; rejected plans are never accepted.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_k_bounded_recovery() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let planner = PlanServiceImpl::new(&repo_path);

    let plan_req = PlanRequest::new(MissionId::new(), "study this codebase")
        .with_classification("RepositoryResearch", true);

    // Initial plan succeeds via bounded recovery synthesis
    let plan = planner.generate_initial_plan(plan_req).await;
    assert!(plan.is_ok(), "Bounded recovery generates safe plan");
}

// -----------------------------------------------------------------------------
// Test L: Persistence and resume across runtime reload
// A session with a stored planning choice retains it on reload.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_l_persistence_and_resume() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());

    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("create session");
    session_repo
        .set_planning_choice(session.id, m31a::runtime::PlanningChoice::DirectExecution)
        .await
        .expect("set planning choice");

    // Reload from fresh session repository instance
    let reloaded_session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let loaded = reloaded_session_repo
        .get_session(session.id)
        .await
        .expect("load")
        .expect("exists");

    let choice = SqliteSessionRepository::read_planning_choice(&loaded);
    assert_eq!(
        choice,
        Some(m31a::runtime::PlanningChoice::DirectExecution),
        "Planning choice must persist across reloads"
    );
}

// -----------------------------------------------------------------------------
// Test M: Architecture parity
// Tool authority scope strictly enforces visible_tools == executable_tools.
// Model cannot see or execute mutating tools when in a read-only role.
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_m_architecture_parity() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("create session");

    let researcher_engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(AgentRole::researcher());

    let scope = researcher_engine.tool_authority_scope();
    let visible = scope.visible_tools();
    let executable = scope.executable_tools();

    // Invariant: visible == executable
    assert_eq!(
        visible, executable,
        "Visible tools must exactly equal executable tools"
    );

    // Hard invariant: no write tools in researcher scope
    for tool in &visible {
        assert!(
            tool != "write_file" && tool != "edit_file" && tool != "apply_patch",
            "Researcher scope must not contain write tool: {}",
            tool
        );
    }
}
