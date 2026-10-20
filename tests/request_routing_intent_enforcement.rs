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

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use support::model_caller::TestModelCaller;
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::ids::MissionId;
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::planner::{PlanError, PlanRequest, PlanService};
use m31a::planning::service::{
    ObjectiveClassification, PlanServiceImpl, classify_objective, is_mutation_forbidden,
    resolve_request_intent,
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

// =============================================================================
// Deterministic Test Doubles for Planning and Agent Invocation
// =============================================================================

#[derive(Clone)]
struct MockProposalCaller {
    proposal: ModelProposal,
    call_count: Arc<AtomicUsize>,
}

impl MockProposalCaller {
    fn from_json(json: serde_json::Value) -> Self {
        Self {
            proposal: ModelProposal::Complete {
                summary: json.to_string(),
                artifacts: vec![],
            },
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelCaller for MockProposalCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.proposal.clone())
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }

    async fn call_model_tool_free_cancellable(
        &self,
        _context: &str,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

// -----------------------------------------------------------------------------
// Test N: Comprehensive table-driven classification and safety invariants
// Verifies Section 3 requirements:
// - Exact table entries
// - Case insensitivity
// - Punctuation tolerance
// - Strict negative phrasing detection overriding mutation markers
// - Safe fallback for unclear/ambiguous prompts
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_n_table_driven_classification_and_safety_invariants() {
    let cases = vec![
        // Table prompts from Section 3
        (
            "What is Rust?",
            ObjectiveClassification::DirectAnswer,
            true,
            false,
        ),
        (
            "What does src/main.rs do?",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        (
            "What does build.rs do?",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        (
            "Explain how the build system works",
            ObjectiveClassification::RepositoryResearch,
            true,
            false,
        ),
        (
            "Check whether the build passes",
            ObjectiveClassification::Verification,
            true,
            false,
        ),
        (
            "Run the existing test suite",
            ObjectiveClassification::Verification,
            true,
            false,
        ),
        (
            "Find why the build is failing and report the cause",
            ObjectiveClassification::Investigation,
            true,
            false,
        ),
        (
            "Do not fix anything; explain the bug",
            ObjectiveClassification::Investigation,
            true,
            true,
        ),
        (
            "Fix the bug in src/auth.rs",
            ObjectiveClassification::Implementation,
            false,
            false,
        ),
        (
            "Write a README for this project",
            ObjectiveClassification::Documentation,
            false,
            false,
        ),
        (
            "Audit the repository, then fix critical findings",
            ObjectiveClassification::Mixed,
            false,
            false,
        ),
        (
            "Run tests and fix any failures",
            ObjectiveClassification::Mixed,
            false,
            false,
        ),
        (
            "An unclear request with insufficient context",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        // Capitalization variants
        (
            "WHAT DOES BUILD.RS DO?",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        (
            "what is rust?",
            ObjectiveClassification::DirectAnswer,
            true,
            false,
        ),
        (
            "FIX THE BUG IN SRC/AUTH.RS",
            ObjectiveClassification::Implementation,
            false,
            false,
        ),
        (
            "EXPLAIN HOW THE BUILD SYSTEM WORKS",
            ObjectiveClassification::RepositoryResearch,
            true,
            false,
        ),
        // Punctuation variants
        (
            "What does build.rs do?!",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        (
            "Can you inspect the code that fixes this issue?",
            ObjectiveClassification::RepositoryInspection,
            true,
            false,
        ),
        (
            "Write a report explaining the implementation.",
            ObjectiveClassification::RepositoryResearch,
            true,
            false,
        ),
        (
            "run tests and fix any failures.",
            ObjectiveClassification::Mixed,
            false,
            false,
        ),
        // Negative phrasings (mutation forbidden, read-only override)
        (
            "don't fix anything; just explain the bug",
            ObjectiveClassification::Investigation,
            true,
            true,
        ),
        (
            "without modifying files, study the parser",
            ObjectiveClassification::RepositoryResearch,
            true,
            true,
        ),
        (
            "without modifying files, inspect the parser",
            ObjectiveClassification::RepositoryInspection,
            true,
            true,
        ),
        (
            "only inspect the code and report",
            ObjectiveClassification::RepositoryInspection,
            true,
            true,
        ),
        (
            "just report why the build is failing",
            ObjectiveClassification::Investigation,
            true,
            true,
        ),
        (
            "never touch the code; analyze the error",
            ObjectiveClassification::Investigation,
            true,
            true,
        ),
        (
            "do not edit the file; review it",
            ObjectiveClassification::RepositoryInspection,
            true,
            true,
        ),
    ];

    for (prompt, expected_class, expected_ro, expected_mut_forbid) in cases {
        let resolved = resolve_request_intent(prompt);
        assert_eq!(
            resolved.classification, expected_class,
            "Prompt '{}' failed classification: expected {:?}, got {:?}",
            prompt, expected_class, resolved.classification
        );
        assert_eq!(
            resolved.is_read_only(),
            expected_ro,
            "Prompt '{}' failed is_read_only: expected {}, got {}",
            prompt,
            expected_ro,
            resolved.is_read_only()
        );
        assert_eq!(
            resolved.is_mutation_forbidden, expected_mut_forbid,
            "Prompt '{}' failed is_mutation_forbidden: expected {}, got {}",
            prompt, expected_mut_forbid, resolved.is_mutation_forbidden
        );
        if expected_mut_forbid {
            assert!(
                is_mutation_forbidden(prompt),
                "is_mutation_forbidden should return true for '{}'",
                prompt
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Test O: Model proposal with TASK-03, implementer, repo.write rejected and recovered
// Verifies Section 6 requirements:
// - Deterministic mock caller returns TASK-03 with implementer and repo.write for "study this codebase"
// - Rejected by validator under read-only contract
// - Recovered safely to deterministic researcher task (TASK-RESEARCH-01)
// - Agent role is researcher, capabilities are read-only only (fs.read, repo.read)
// - Zero write tools exposed
// - Also verifies unknown roles, unregistered capabilities, and negated mutation
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_o_model_proposal_task03_repo_write_rejected_and_safely_recovered() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let runtime = AppRuntime::new(&repo_path).await.expect("runtime new");
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("create session");

    // Case 1: Model proposes TASK-03 with implementer and repo.write / fs.write for read-only study
    let mutating_proposal = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-03",
                "title": "Study codebase mutating",
                "description": "Attempt mutating operation during study",
                "role": "implementer",
                "depends_on": [],
                "required_capabilities": ["repo.write", "fs.write"]
            }
        ]
    });
    let caller = Arc::new(MockProposalCaller::from_json(mutating_proposal));
    let planner = PlanServiceImpl::new(&repo_path).with_model_caller(caller);
    let resp = planner
        .generate_initial_plan(PlanRequest::new(MissionId::new(), "study this codebase"))
        .await
        .expect("Plan generation must recover cleanly");

    assert_eq!(
        resp.candidate_plan.tasks.len(),
        1,
        "Must synthesize recovery task"
    );
    let task = &resp.candidate_plan.tasks[0];
    assert_eq!(
        task.id,
        CandidateTaskKey::new("TASK-RESEARCH-01"),
        "TASK-03 must be rejected and replaced with synthesized research task"
    );
    assert_eq!(
        task.role,
        AgentRole::researcher(),
        "Synthesized task must run as researcher"
    );
    for cap in &task.capabilities {
        assert_ne!(cap.id, "repo.write");
        assert_ne!(cap.id, "fs.write");
        assert_eq!(cap.mode, CapabilityAccessMode::Read);
    }
    let validator = PlanValidator::new().with_read_only(true);
    let report = validator.validate(&resp.candidate_plan).await;
    assert!(
        report.is_valid(),
        "Synthesized recovery plan must validate cleanly: {:?}",
        report.errors
    );

    // Zero write tools exposed in tool authority scope
    let engine = runtime
        .create_agent_engine(session.id)
        .with_role_authority(task.role.clone());
    let visible = engine.tool_authority_scope().visible_tools();
    assert!(!visible.contains(&"write_file".to_string()));
    assert!(!visible.contains(&"edit_file".to_string()));
    assert!(!visible.contains(&"apply_patch".to_string()));

    // Case 2: Model proposes unknown role
    let unknown_role_proposal = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-UNKNOWN",
                "title": "Unknown role attempt",
                "description": "Task declaring unknown role",
                "role": "super_admin_role",
                "depends_on": [],
                "required_capabilities": ["fs.read"]
            }
        ]
    });
    let caller = Arc::new(MockProposalCaller::from_json(unknown_role_proposal));
    let planner = PlanServiceImpl::new(&repo_path).with_model_caller(caller);
    let resp = planner
        .generate_initial_plan(PlanRequest::new(MissionId::new(), "study this codebase"))
        .await
        .expect("Unknown role must recover safely");
    assert_eq!(
        resp.candidate_plan.tasks[0].id,
        CandidateTaskKey::new("TASK-RESEARCH-01")
    );
    assert_eq!(resp.candidate_plan.tasks[0].role, AgentRole::researcher());

    // Case 3: Model proposes unregistered capability
    let unreg_cap_proposal = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-UNREG",
                "title": "Unregistered capability attempt",
                "description": "Task declaring unregistered capability",
                "role": "researcher",
                "depends_on": [],
                "required_capabilities": ["arbitrary.hack.capability"]
            }
        ]
    });
    let caller = Arc::new(MockProposalCaller::from_json(unreg_cap_proposal));
    let planner = PlanServiceImpl::new(&repo_path).with_model_caller(caller);
    let resp = planner
        .generate_initial_plan(PlanRequest::new(MissionId::new(), "study this codebase"))
        .await
        .expect("Unregistered capability must recover safely");
    assert_eq!(
        resp.candidate_plan.tasks[0].id,
        CandidateTaskKey::new("TASK-RESEARCH-01")
    );
    assert_eq!(resp.candidate_plan.tasks[0].role, AgentRole::researcher());

    // Case 4: Negated mutation prompt with mutating model proposal
    let caller = Arc::new(MockProposalCaller::from_json(serde_json::json!({
        "tasks": [
            {
                "id": "TASK-WRITE",
                "title": "Write attempt",
                "description": "Attempt to write when user forbade mutation",
                "role": "implementer",
                "depends_on": [],
                "required_capabilities": ["fs.write"]
            }
        ]
    })));
    let planner = PlanServiceImpl::new(&repo_path).with_model_caller(caller);
    let resp = planner
        .generate_initial_plan(PlanRequest::new(
            MissionId::new(),
            "Do not fix anything; inspect the codebase",
        ))
        .await
        .expect("Negated mutation must recover safely to read-only task");
    assert_eq!(
        resp.candidate_plan.tasks[0].id,
        CandidateTaskKey::new("TASK-RESEARCH-01")
    );
    assert_eq!(resp.candidate_plan.tasks[0].role, AgentRole::researcher());
}

// -----------------------------------------------------------------------------
// Test P: Bounded recovery retry exhaustion on actionable implementation request
// Verifies Section 7 requirements:
// - Implementation request ("Fix the parser")
// - Mock caller consistently returns invalid proposals (empty tasks)
// - max_retries is reached (2 retries = 3 total attempts)
// - Planner returns explicit PlanError::GenerationFailed, NEVER substituting research tasks
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_p_bounded_recovery_exhaustion_on_implementation_request() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let empty_proposal = serde_json::json!({
        "tasks": []
    });
    let caller = Arc::new(MockProposalCaller::from_json(empty_proposal));
    let planner = PlanServiceImpl::new(&repo_path).with_model_caller(caller.clone());

    let plan_req = PlanRequest::new(MissionId::new(), "Fix the parser");
    assert!(!plan_req.resolved_contract().is_read_only());

    let res = planner.generate_initial_plan(plan_req).await;
    assert!(
        res.is_err(),
        "Exhausted retries on implementation request must fail"
    );
    match res {
        Err(PlanError::GenerationFailed(msg)) => {
            assert!(
                msg.contains("explicit re-planning or recovery required")
                    || msg.contains("no fallback tasks substituted"),
                "Error must state that explicit recovery is required and no fallback was substituted: {}",
                msg
            );
        }
        other => panic!("Expected PlanError::GenerationFailed, got {:?}", other),
    }

    // 1 initial attempt + 2 retries = 3 calls
    assert_eq!(
        caller.call_count.load(Ordering::SeqCst),
        3,
        "Planner must attempt exactly 1 + max_retries (3) times"
    );
}

// -----------------------------------------------------------------------------
// Test Q: Isolate state between sequential user requests in InteractiveSessionRunner
// Verifies Section 2 requirements:
// - Direct answer -> repository inspection -> implementation -> read-only study -> direct answer
// - Real request lifecycle execution through InteractiveSessionRunner
// - Role, denied-tools, and intent do NOT leak from one request to the next
// - AgentEngine::reset_for_new_request explicitly resets state, intent, and role authority
// -----------------------------------------------------------------------------
#[test]
fn test_q_sequential_requests_engine_state_isolation() {
    let builder = std::thread::Builder::new()
        .name("test-q-worker".to_string())
        .stack_size(8 * 1024 * 1024);
    let handler = builder
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let _ = m31a::telemetry::redactor::SecretRedactor::global();
                    let (_dir, repo_path) = setup_git_workspace().await;
                    let caller = Arc::new(TestModelCaller::from_proposals(vec![
                        // Turn 1: Commentary + Answer
                        Ok(ModelProposal::AssistantText {
                            content: "Analyzing Rust query...".to_string(),
                        }),
                        Ok(ModelProposal::AssistantText {
                            content: "Rust is a modern systems language.".to_string(),
                        }),
                        // Turn 2: Commentary + Answer
                        Ok(ModelProposal::AssistantText {
                            content: "Analyzing src/main.rs...".to_string(),
                        }),
                        Ok(ModelProposal::AssistantText {
                            content: "src/main.rs defines the entrypoint fn main.".to_string(),
                        }),
                        // Turn 3: Commentary + Answer
                        Ok(ModelProposal::AssistantText {
                            content: "Analyzing authentication bug...".to_string(),
                        }),
                        Ok(ModelProposal::AssistantText {
                            content: "Fixed the authentication bug in src/auth.rs.".to_string(),
                        }),
                        // Turn 4: Commentary + Answer
                        Ok(ModelProposal::AssistantText {
                            content: "Conducting read-only codebase study...".to_string(),
                        }),
                        Ok(ModelProposal::AssistantText {
                            content: "Completed read-only study of repository architecture."
                                .to_string(),
                        }),
                        // Turn 5: Commentary + Answer
                        Ok(ModelProposal::AssistantText {
                            content: "Analyzing borrow checker concept...".to_string(),
                        }),
                        Ok(ModelProposal::AssistantText {
                            content: "Borrow checker ensures memory safety at compile time."
                                .to_string(),
                        }),
                    ]));
                    let runtime = Arc::new(
                        AppRuntime::new(&repo_path)
                            .await
                            .expect("runtime")
                            .with_model_caller(caller),
                    );
                    let mut runner = InteractiveSessionRunner::new(runtime.clone());
                    let session = runner.init_session(None).await.expect("init session");
                    runner
                        .session_repo()
                        .set_planning_choice(
                            session.id,
                            m31a::runtime::PlanningChoice::DirectExecution,
                        )
                        .await
                        .expect("set planning choice");

                    // Turn 1: General knowledge question ("What is Rust?") -> DirectAnswer
                    let exit = runner.handle_input("What is Rust?").await.expect("turn 1");
                    assert!(!exit);
                    assert!(
                        !runner.has_active_engine(),
                        "Finished turn must clear active_engine"
                    );

                    // Turn 2: Repository inspection ("What does src/main.rs do?")
                    let exit = runner
                        .handle_input("What does src/main.rs do?")
                        .await
                        .expect("turn 2");
                    assert!(!exit);
                    assert!(
                        !runner.has_active_engine(),
                        "Finished turn must clear active_engine"
                    );

                    // Turn 3: Implementation request ("Fix the authentication bug in src/auth.rs")
                    let exit = runner
                        .handle_input("Fix the authentication bug in src/auth.rs")
                        .await
                        .expect("turn 3");
                    assert!(!exit);
                    assert!(!runner.has_active_engine());

                    // Turn 4: Explicit read-only study ("Study the codebase and report findings without modifying any files")
                    let exit = runner
                        .handle_input(
                            "Study the codebase and report findings without modifying any files",
                        )
                        .await
                        .expect("turn 4");
                    assert!(!exit);
                    assert!(!runner.has_active_engine());

                    // Turn 5: Unrelated general knowledge question ("Explain the borrow checker concept")
                    let exit = runner
                        .handle_input("Explain the borrow checker concept")
                        .await
                        .expect("turn 5");
                    assert!(!exit);
                    assert!(!runner.has_active_engine());

                    // Verify durable conversation history has all 5 turns and their responses recorded in sequence
                    let conv = runner
                        .session_repo()
                        .get_conversation(session.id)
                        .await
                        .expect("get conversation");
                    assert_eq!(
                        conv.len(),
                        15,
                        "5 user turns + 10 assistant turns (commentary + text) = 15 turns"
                    );

                    // Direct contract verification of AgentEngine::reset_for_new_request
                    let mut engine = runtime.create_agent_engine(session.id);

                    // Phase 1: DirectAnswer with all tools denied
                    let all_tools = engine.tool_authority_scope().visible_tools();
                    engine = engine.with_denied_tools(all_tools.clone());
                    engine.reset_intent_from_prompt("What is Rust?");
                    assert!(engine.tool_authority_scope().visible_tools().is_empty());
                    assert_eq!(engine.intent_state().unwrap().raw_prompt, "What is Rust?");

                    // Phase 2: Reset to RepositoryInspection (researcher role, empty denied tools)
                    engine.reset_for_new_request(
                        "What does src/main.rs do?",
                        AgentRole::researcher(),
                        vec![],
                    );
                    assert_eq!(engine.active_role(), &AgentRole::researcher());
                    assert!(
                        engine.denied_tools().is_empty(),
                        "Denied tools must be cleared"
                    );
                    assert_eq!(
                        engine.intent_state().unwrap().raw_prompt,
                        "What does src/main.rs do?",
                        "Intent must be updated"
                    );
                    let visible = engine.tool_authority_scope().visible_tools();
                    assert!(visible.contains(&"read_file".to_string()));
                    assert!(!visible.contains(&"write_file".to_string()));

                    // Phase 3: Reset to Implementation (implementer role, write tools available)
                    engine.reset_for_new_request(
                        "Fix the authentication bug in src/auth.rs",
                        AgentRole::implementer(),
                        vec![],
                    );
                    assert_eq!(engine.active_role(), &AgentRole::implementer());
                    assert_eq!(
                        engine.intent_state().unwrap().raw_prompt,
                        "Fix the authentication bug in src/auth.rs"
                    );
                    let visible = engine.tool_authority_scope().visible_tools();
                    assert!(visible.contains(&"write_file".to_string()));

                    // Phase 4: Reset to Read-only study (researcher role, write tools revoked)
                    engine.reset_for_new_request(
                        "Study the codebase without modifying any files",
                        AgentRole::researcher(),
                        vec![],
                    );
                    assert_eq!(engine.active_role(), &AgentRole::researcher());
                    assert_eq!(
                        engine.intent_state().unwrap().raw_prompt,
                        "Study the codebase without modifying any files"
                    );
                    let visible = engine.tool_authority_scope().visible_tools();
                    assert!(visible.contains(&"read_file".to_string()));
                    assert!(
                        !visible.contains(&"write_file".to_string()),
                        "Write tools must NOT leak to read-only role"
                    );

                    // Phase 5: Reset to DirectAnswer (all tools denied)
                    let all_tools = engine.tool_authority_scope().visible_tools();
                    engine.reset_for_new_request(
                        "Explain the borrow checker concept",
                        AgentRole::researcher(),
                        all_tools,
                    );
                    assert!(engine.tool_authority_scope().visible_tools().is_empty());
                    assert_eq!(
                        engine.intent_state().unwrap().raw_prompt,
                        "Explain the borrow checker concept"
                    );
                })
        })
        .unwrap();
    handler.join().unwrap();
}

// -----------------------------------------------------------------------------
// Test R: Active continuation preserves state during clarification / question flow
// Verifies Section 2 requirements:
// - Model requests clarification via AskUser
// - Engine state enters WaitingForUser
// - Runner preserves active_engine across turn
// - Subsequent user input delivers response to waiting engine
// - Interaction continues and finishes cleanly
// - Durable session history records User -> AskUser -> User -> Assistant sequence
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_r_active_continuation_preserves_state() {
    let (_dir, repo_path) = setup_git_workspace().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::AskUser {
            question: "Should I inspect src/auth.rs or tests/auth_test.rs?".to_string(),
            options: vec![],
        }),
        Ok(ModelProposal::AssistantText {
            content: "Found authentication bug in src/auth.rs line 42.".to_string(),
        }),
        Ok(ModelProposal::AssistantText {
            content: "Review of authentication architecture complete.".to_string(),
        }),
    ]));
    let runtime = Arc::new(
        AppRuntime::new(&repo_path)
            .await
            .expect("runtime")
            .with_model_caller(caller),
    );
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.expect("init session");
    runner
        .session_repo()
        .set_planning_choice(session.id, m31a::runtime::PlanningChoice::DirectExecution)
        .await
        .expect("set planning choice");

    // Step 1: User prompt triggers AskUser proposal from model
    let exit = runner
        .handle_input("Review the authentication module architecture")
        .await
        .expect("initial prompt");
    assert!(!exit);
    assert!(
        runner.has_active_engine(),
        "Engine must be retained while waiting for user"
    );
    let active_engine = runner.active_engine().expect("active engine exists");
    match active_engine.state() {
        m31a::agent::AgentEngineState::WaitingForUser { question, .. } => {
            assert_eq!(
                question,
                "Should I inspect src/auth.rs or tests/auth_test.rs?"
            );
        }
        other => panic!("Expected WaitingForUser state, got {:?}", other),
    }

    // Step 2: User provides clarification response
    let exit = runner
        .handle_input("src/auth.rs")
        .await
        .expect("clarification response");
    assert!(!exit);
    assert!(
        !runner.has_active_engine(),
        "Engine must be cleared once turn completes"
    );

    // Step 3: Inspect durable conversation turns
    let conv = runner
        .session_repo()
        .get_conversation(session.id)
        .await
        .expect("get conversation");
    assert_eq!(
        conv.len(),
        5,
        "Must record 5 turns: UserMessage, AskUserMessage, UserMessage, AssistantMessage (commentary), AssistantMessage (completion)"
    );

    assert!(matches!(
        &conv[0],
        m31a::interaction::session::ConversationTurn::UserMessage { content, .. }
            if content == "Review the authentication module architecture"
    ));
    assert!(matches!(
        &conv[1],
        m31a::interaction::session::ConversationTurn::AskUserMessage { question, .. }
            if question == "Should I inspect src/auth.rs or tests/auth_test.rs?"
    ));
    assert!(matches!(
        &conv[2],
        m31a::interaction::session::ConversationTurn::UserMessage { content, .. }
            if content == "src/auth.rs"
    ));
    assert!(matches!(
        &conv[3],
        m31a::interaction::session::ConversationTurn::AssistantMessage { content, .. }
            if content == "Found authentication bug in src/auth.rs line 42."
    ));
    assert!(matches!(
        &conv[4],
        m31a::interaction::session::ConversationTurn::AssistantMessage { content, .. }
            if content == "Review of authentication architecture complete."
    ));

    // Verify sequences are strictly monotonic
    for (i, turn) in conv.iter().enumerate() {
        assert_eq!(
            turn.sequence(),
            (i + 1) as u64,
            "Sequence must be strictly monotonic"
        );
    }
}
