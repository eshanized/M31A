//! Integration test suite for Workstream D: Planning Uncapping + Prompt Catalog Consolidation.
//!
//! Verifies:
//! - Test A: One-task bug fix
//! - Test B: Multi-file feature (>= 2 tasks)
//! - Test C: Dependency-sensitive multi-task change (Task A -> B -> C)
//! - Test D: Test-failure + repair decomposition (diagnose -> patch -> verify)
//! - Test E: Read-only planning producing 0 execution tasks
//! - Test F: Cyclic model plan rejected
//! - Test G: Duplicate task IDs rejected
//! - Test H: Invalid dependency rejected
//! - Test I: Invalid role rejected
//! - Test J: Capability mismatch rejected (read-only role requesting write)
//! - Test K: Bounded retry / timeout constraints enforced
//! - Test L: Degraded model output fails explicitly (no fabricated fallback)
//! - Test M: PromptCatalog actually used by production planning
//! - Test N: Hardcoded legacy prompt no longer reachable
//! - Test O: Role-specific prompt composition
//! - Test P: Prompt content hash / provenance
//! - Test Q: Planner output produces a real TaskGraph
//! - Production Path Scenario: Multi-task plan executed through production spine

use async_trait::async_trait;
use m31a::agent::AgentRole;
use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::context::compiler::ProductionContextCompiler;
use m31a::dag::materializer::TaskGraphMaterializer;
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::context::{CompiledContext, ContextCompilationRequest, ContextCompiler};
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::model::types::ChatMessage;
use m31a::planning::service::{PlanServiceImpl, is_read_only_objective};
use m31a::planning::validation::{PlanValidator, ValidationError};
use m31a::prompt::{
    InMemoryPromptCatalog, PromptCatalog, PromptContract, PromptContractMetadata, PromptError,
    render_prompt,
};
use m31a::runtime::AppRuntime;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::engine::WorkflowStartRequest;
use m31a::workflow::provenance::WorkflowProvenance;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowMode, WorkflowRunState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

/// Mock ModelCaller that returns a canned ModelProposal for planning or execution.
#[derive(Default)]
struct CannedModelCaller {
    pub call_count: AtomicUsize,
    pub proposal_summary: String,
    pub recorded_contexts: std::sync::Mutex<Vec<String>>,
}

impl CannedModelCaller {
    fn new(summary: impl Into<String>) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            proposal_summary: summary.into(),
            recorded_contexts: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ModelCaller for CannedModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.recorded_contexts
            .lock()
            .unwrap()
            .push(context.to_string());
        Ok(ModelProposal::Complete {
            summary: self.proposal_summary.clone(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let ctx = format!("{:?}", compiled.messages);
        self.recorded_contexts.lock().unwrap().push(ctx);
        Ok(ModelProposal::Complete {
            summary: self.proposal_summary.clone(),
            artifacts: vec![],
        })
    }
}

/// Instrumented PromptCatalog wrapper that tracks requested template keys.
struct TrackingPromptCatalog {
    inner: InMemoryPromptCatalog,
    requested_keys: std::sync::Mutex<Vec<String>>,
}

impl TrackingPromptCatalog {
    fn new() -> Self {
        Self {
            inner: InMemoryPromptCatalog::with_builtins(),
            requested_keys: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl PromptCatalog for TrackingPromptCatalog {
    fn get(&self, id: &str, version: u32) -> Result<&PromptContract, PromptError> {
        self.requested_keys.lock().unwrap().push(id.to_string());
        self.inner.get(id, version)
    }

    fn contains(&self, id: &str, version: u32) -> bool {
        self.inner.contains(id, version)
    }

    fn list(&self) -> Vec<PromptContractMetadata> {
        self.inner.list()
    }
}

fn setup_git_fixture(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(dir)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "test@m31a.local"])
        .current_dir(dir)
        .status()
        .expect("git config user.email failed");

    std::fs::write(dir.join(".gitignore"), "/target\n.m31a\n").unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture_package\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn placeholder() -> bool { true }\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial fixture"])
        .current_dir(dir)
        .status()
        .expect("git commit failed");
}

fn build_test_workflow(
    id: &str,
    steps: Vec<WorkflowStepDefinition>,
    default_strategy: RecoveryStrategy,
) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Test workflow".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: default_strategy,
    };
    def.validate().unwrap();

    let topological_order = def.steps.iter().map(|s| s.key.clone()).collect();
    let provenance = WorkflowProvenance::new(
        id,
        1,
        "test-hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );

    CompiledWorkflow {
        definition: def,
        provenance,
        topological_order,
    }
}

// =========================================================================
// Test A: One-task bug fix
// =========================================================================
#[tokio::test]
async fn test_condition_a_one_task_bug_fix() {
    let dir = tempdir().unwrap();
    let plan_json = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-01",
                "title": "Fix parser off-by-one bug",
                "description": "Adjust token scanner index bounds in src/parser.rs",
                "role": "implementer",
                "depends_on": [],
                "required_capabilities": ["fs.read", "fs.write"]
            }
        ]
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    let req = PlanRequest::new(
        MissionId::new(),
        "Fix parser off-by-one bug in src/parser.rs",
    );

    let resp = planner.generate_initial_plan(req).await.unwrap();
    assert_eq!(resp.task_count, 1);
    assert_eq!(resp.candidate_plan.tasks.len(), 1);
    assert_eq!(resp.candidate_plan.tasks[0].id.as_str(), "TASK-01");
    assert_eq!(resp.candidate_plan.tasks[0].role, AgentRole::implementer());

    let validator = PlanValidator::new();
    let report = validator.validate(&resp.candidate_plan).await;
    assert!(report.is_valid(), "Report errors: {:?}", report.errors);
}

// =========================================================================
// Test B: Multi-file feature spanning >= 2 tasks
// =========================================================================
#[tokio::test]
async fn test_condition_b_multi_file_feature() {
    let dir = tempdir().unwrap();
    let plan_json = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-01",
                "title": "Implement session storage",
                "description": "Create in-memory session store in src/session.rs",
                "role": "implementer",
                "depends_on": [],
                "required_capabilities": ["fs.read", "fs.write"]
            },
            {
                "id": "TASK-02",
                "title": "Implement auth middleware",
                "description": "Connect auth middleware to session store in src/middleware.rs",
                "role": "implementer",
                "depends_on": ["TASK-01"],
                "required_capabilities": ["fs.read", "fs.write"]
            }
        ]
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    let req = PlanRequest::new(
        MissionId::new(),
        "Implement session storage and auth middleware across multiple files",
    );

    let resp = planner.generate_initial_plan(req).await.unwrap();
    assert_eq!(
        resp.task_count, 2,
        "CandidatePlan must preserve >= 2 tasks without truncation"
    );
    assert_eq!(resp.candidate_plan.tasks.len(), 2);
    assert_eq!(resp.candidate_plan.tasks[1].depends_on.len(), 1);
    assert_eq!(
        resp.candidate_plan.tasks[1].depends_on[0].as_str(),
        "TASK-01"
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&resp.candidate_plan).await;
    assert!(
        report.is_valid(),
        "Multi-task plan must pass validation: {:?}",
        report.errors
    );
}

// =========================================================================
// Test C: Dependency-sensitive multi-task change
// =========================================================================
#[tokio::test]
async fn test_condition_c_dependency_sensitive_multi_task() {
    let plan = CandidatePlan::new(
        "PLAN-DEP",
        "Pipeline implementation: A -> B -> C",
        vec![
            CandidateTask {
                id: CandidateTaskKey::new("TASK-01"),
                objective: "Step A: Base library types".to_string(),
                description: None,
                depends_on: vec![],
                capabilities: vec![CapabilityRequirement::new(
                    "fs.read",
                    CapabilityAccessMode::Read,
                )],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-02"),
                objective: "Step B: Implement algorithms".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-01")],
                capabilities: vec![CapabilityRequirement::new(
                    "fs.read",
                    CapabilityAccessMode::Read,
                )],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-03"),
                objective: "Step C: Expose public API".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-02")],
                capabilities: vec![CapabilityRequirement::new(
                    "fs.read",
                    CapabilityAccessMode::Read,
                )],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
        ],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(
        report.is_valid(),
        "DAG with valid linear dependencies must pass: {:?}",
        report.errors
    );

    let dir = tempdir().unwrap();
    setup_git_fixture(dir.path());
    let runtime = AppRuntime::new(dir.path()).await.unwrap();

    let mission_repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
        runtime.pool().clone(),
    );
    let mission_id = MissionId::new();
    let mission = m31a::state::mission::Mission::new(mission_id, "Test mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let materializer = TaskGraphMaterializer::new(runtime.pool().clone());
    let graph = materializer
        .materialize(mission_id, &plan)
        .await
        .expect("materialize DAG plan failed");

    assert_eq!(graph.tasks.len(), 3);
    assert_eq!(graph.wave_tiers.len(), 3);
    // Tier 0 has TASK-01, Tier 1 has TASK-02, Tier 2 has TASK-03
    assert_eq!(graph.wave_tiers.get(&0).unwrap().len(), 1);
    assert_eq!(graph.wave_tiers.get(&1).unwrap().len(), 1);
    assert_eq!(graph.wave_tiers.get(&2).unwrap().len(), 1);
}

// =========================================================================
// Test D: Test-failure + repair decomposition (diagnose -> patch -> verify)
// =========================================================================
#[tokio::test]
async fn test_condition_d_test_failure_repair_decomposition() {
    let dir = tempdir().unwrap();
    let plan_json = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-01",
                "title": "Diagnose failure root cause",
                "description": "Examine test output and isolate panicking assertion",
                "role": "diagnostician",
                "depends_on": [],
                "required_capabilities": ["fs.read"]
            },
            {
                "id": "TASK-02",
                "title": "Patch implementation",
                "description": "Apply fix to src/lib.rs",
                "role": "implementer",
                "depends_on": ["TASK-01"],
                "required_capabilities": ["fs.read", "fs.write"]
            },
            {
                "id": "TASK-03",
                "title": "Verify fix passes test suite",
                "description": "Run cargo test and record verification evidence",
                "role": "verifier",
                "depends_on": ["TASK-02"],
                "required_capabilities": ["fs.read", "cargo.test", "evidence.record"]
            }
        ]
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    let req = PlanRequest::new(
        MissionId::new(),
        "Test failure: assertion failed `left == right` in tests/parser_test.rs",
    );

    let resp = planner.generate_initial_plan(req).await.unwrap();
    assert_eq!(resp.task_count, 3);
    assert_eq!(
        resp.candidate_plan.tasks[0].role,
        AgentRole::diagnostician()
    );
    assert_eq!(resp.candidate_plan.tasks[1].role, AgentRole::implementer());
    assert_eq!(resp.candidate_plan.tasks[2].role, AgentRole::verifier());
    assert_eq!(
        resp.candidate_plan.tasks[1].depends_on[0].as_str(),
        "TASK-01"
    );
    assert_eq!(
        resp.candidate_plan.tasks[2].depends_on[0].as_str(),
        "TASK-02"
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&resp.candidate_plan).await;
    assert!(
        report.is_valid(),
        "3-stage decomposition must be valid: {:?}",
        report.errors
    );
}

// =========================================================================
// Test E: Read-only planning producing 0 execution tasks
// =========================================================================
#[tokio::test]
async fn test_condition_e_read_only_planning_zero_tasks() {
    let dir = tempdir().unwrap();
    // Model returns empty tasks list for read-only query
    let plan_json = serde_json::json!({
        "tasks": []
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    let read_only_objective = "Analyze codebase architecture and review dependencies";
    assert!(is_read_only_objective(read_only_objective));

    let req = PlanRequest::new(MissionId::new(), read_only_objective);

    let resp = planner.generate_initial_plan(req).await.unwrap();
    assert_eq!(
        resp.task_count, 0,
        "Read-only plan must have 0 execution tasks"
    );
    assert_eq!(resp.candidate_plan.tasks.len(), 0);

    // Validation passes with allow_empty(true)
    let validator = PlanValidator::new().with_allow_empty(true);
    let report = validator.validate(&resp.candidate_plan).await;
    assert!(
        report.is_valid(),
        "Read-only empty plan must pass validation under allow_empty: {:?}",
        report.errors
    );

    // Also verify that a read-only objective yields the semantically safe
    // generic empty plan (no fabricated tasks).
    let read_only_plan = planner
        .generate_initial_plan(PlanRequest::new(MissionId::new(), read_only_objective))
        .await
        .unwrap();
    assert!(
        read_only_plan.candidate_plan.tasks.is_empty(),
        "Read-only objective must yield an empty plan, never fabricated tasks"
    );
}

// =========================================================================
// Test F: Cyclic model plan rejected
// =========================================================================
#[tokio::test]
async fn test_condition_f_cyclic_model_plan_rejected() {
    let plan = CandidatePlan::new(
        "PLAN-CYCLE",
        "Cyclic plan",
        vec![
            CandidateTask {
                id: CandidateTaskKey::new("TASK-01"),
                objective: "Task A".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-02")],
                capabilities: vec![],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-02"),
                objective: "Task B".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-01")],
                capabilities: vec![],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
        ],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(!report.is_valid());
    let has_cycle = report
        .errors
        .iter()
        .any(|e| matches!(e, ValidationError::CycleDetected { .. }));
    assert!(
        has_cycle,
        "Cyclic plan must be rejected with CycleDetected error: {:?}",
        report.errors
    );
}

// =========================================================================
// Test G: Duplicate task IDs rejected
// =========================================================================
#[tokio::test]
async fn test_condition_g_duplicate_task_ids_rejected() {
    let plan = CandidatePlan::new(
        "PLAN-DUP",
        "Plan with duplicate IDs",
        vec![
            CandidateTask {
                id: CandidateTaskKey::new("TASK-01"),
                objective: "First Task-01".to_string(),
                description: None,
                depends_on: vec![],
                capabilities: vec![],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-01"),
                objective: "Duplicate Task-01".to_string(),
                description: None,
                depends_on: vec![],
                capabilities: vec![],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
        ],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(!report.is_valid());
    let has_dup = report
        .errors
        .iter()
        .any(|e| matches!(e, ValidationError::DuplicateTaskId { .. }));
    assert!(
        has_dup,
        "Duplicate task ID must be rejected: {:?}",
        report.errors
    );
}

// =========================================================================
// Test H: Invalid dependency rejected
// =========================================================================
#[tokio::test]
async fn test_condition_h_invalid_dependency_rejected() {
    let plan = CandidatePlan::new(
        "PLAN-MISSING-PREREQ",
        "Missing prerequisite",
        vec![CandidateTask {
            id: CandidateTaskKey::new("TASK-01"),
            objective: "Task with missing dep".to_string(),
            description: None,
            depends_on: vec![CandidateTaskKey::new("TASK-NONEXISTENT")],
            capabilities: vec![],
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::default(),
            ..Default::default()
        }],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(!report.is_valid());
    let has_missing = report
        .errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingPrerequisite { .. }));
    assert!(
        has_missing,
        "Missing dependency must be rejected: {:?}",
        report.errors
    );
}

// =========================================================================
// Test I: Invalid role rejected
// =========================================================================
#[tokio::test]
async fn test_condition_i_invalid_role_rejected() {
    let validator = PlanValidator::new();
    let invalid_role_result = validator.validate_role("TASK-01", "super_admin_god_mode");
    assert!(invalid_role_result.is_err());
    assert!(matches!(
        invalid_role_result.unwrap_err(),
        ValidationError::UnknownRole { .. }
    ));

    let dir = tempdir().unwrap();
    let plan_json = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-01",
                "title": "Unsafe root execution",
                "description": "Bypass limits",
                "role": "super_admin_god_mode",
                "depends_on": [],
                "required_capabilities": []
            }
        ]
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    let req = PlanRequest::new(MissionId::new(), "Perform task");

    let res = planner.generate_initial_plan(req).await;
    assert!(res.is_err(), "Invalid role must fail planning generation");
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("UnknownRole") || err_str.contains("super_admin_god_mode"),
        "Error was: {}",
        err_str
    );
}

// =========================================================================
// Test J: Capability mismatch rejected
// =========================================================================
#[tokio::test]
async fn test_condition_j_capability_mismatch_rejected() {
    // Diagnostician is strictly read-only; requesting Write capability must be rejected
    let plan = CandidatePlan::new(
        "PLAN-CAP-MISMATCH",
        "Diagnostician requesting write",
        vec![CandidateTask {
            id: CandidateTaskKey::new("TASK-01"),
            objective: "Diagnose and mutate".to_string(),
            description: None,
            depends_on: vec![],
            capabilities: vec![CapabilityRequirement::new(
                "filesystem.write",
                CapabilityAccessMode::Write,
            )],
            role: AgentRole::diagnostician(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::default(),
            ..Default::default()
        }],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(!report.is_valid());
    let has_policy_conflict = report
        .errors
        .iter()
        .any(|e| matches!(e, ValidationError::PolicyConflict { .. }));
    assert!(
        has_policy_conflict,
        "Read-only role requesting mutating capability must trigger PolicyConflict: {:?}",
        report.errors
    );
}

// =========================================================================
// Test K: Bounded retry / timeout constraints enforced
// =========================================================================
#[tokio::test]
async fn test_condition_k_bounded_retry_timeout_constraints() {
    // Timeout exceeding 7200s or steps exceeding 100
    let invalid_estimate = ResourceEstimate::new(500, 10_000, 50_000, 1.0);
    assert!(invalid_estimate.validate().is_err());

    let plan = CandidatePlan::new(
        "PLAN-BOUNDS",
        "Unbounded resource plan",
        vec![CandidateTask {
            id: CandidateTaskKey::new("TASK-01"),
            objective: "Unbounded execution".to_string(),
            description: None,
            depends_on: vec![],
            capabilities: vec![],
            role: AgentRole::implementer(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::new(200, 10_000, 50_000, 1.0),
            ..Default::default()
        }],
    );

    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(!report.is_valid());
    let has_bounds_exceeded = report
        .errors
        .iter()
        .any(|e| matches!(e, ValidationError::BoundsExceeded { .. }));
    assert!(
        has_bounds_exceeded,
        "Exceeding duration bounds must be rejected: {:?}",
        report.errors
    );
}

// =========================================================================
// Test L: Degraded model output fails explicitly (no fabricated fallback)
// =========================================================================
#[tokio::test]
async fn test_condition_l_degraded_model_fails_explicitly() {
    let dir = tempdir().unwrap();
    // Model returns broken malformed JSON
    let caller = Arc::new(CannedModelCaller::new("NOT VALID JSON AT ALL {}}"));
    let planner = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

    // 1. Implementation objective -> explicit GenerationFailed, never a
    // fabricated 3-task plan (Phase 27, Test H: no fake fallback).
    let req_impl = PlanRequest::new(MissionId::new(), "Refactor database query engine");
    let err = planner
        .generate_initial_plan(req_impl)
        .await
        .expect_err("Malformed model output must fail explicitly");
    let err_str = err.to_string();
    assert!(
        err_str.contains("malformed") && err_str.contains("no fallback tasks substituted"),
        "Error must name the cause and the no-fabrication policy: {}",
        err_str
    );

    // 2. Read-only objective with malformed output also fails explicitly:
    // the runtime cannot distinguish "0 tasks intended" from corruption.
    let req_read = PlanRequest::new(MissionId::new(), "Explain system architecture");
    assert!(
        planner.generate_initial_plan(req_read).await.is_err(),
        "Malformed model output must fail even for read-only inquiries"
    );
}

// =========================================================================
// Test M: PromptCatalog actually used by production planning
// =========================================================================
#[tokio::test]
async fn test_condition_m_prompt_catalog_used_by_production_planning() {
    let dir = tempdir().unwrap();
    let tracking_catalog = Arc::new(TrackingPromptCatalog::new());

    let plan_json = serde_json::json!({
        "tasks": [
            {
                "id": "TASK-01",
                "title": "Generic task",
                "description": "Perform work",
                "role": "implementer",
                "depends_on": [],
                "required_capabilities": ["fs.read"]
            }
        ]
    });

    let caller = Arc::new(CannedModelCaller::new(plan_json.to_string()));
    let planner = PlanServiceImpl::new(dir.path())
        .with_model_caller(caller.clone())
        .with_prompt_catalog(tracking_catalog.clone());

    let req = PlanRequest::new(MissionId::new(), "Implement feature X");

    let _resp = planner.generate_initial_plan(req).await.unwrap();

    let keys = tracking_catalog.requested_keys.lock().unwrap().clone();
    assert!(
        keys.contains(&"planning.decompose".to_string()),
        "Production planning MUST request 'planning.decompose' from PromptCatalog (Condition M). Got: {:?}",
        keys
    );
}

// =========================================================================
// Test N: Hardcoded legacy prompt no longer reachable
// =========================================================================
#[tokio::test]
async fn test_condition_n_hardcoded_legacy_prompt_unreachable() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let contract = catalog.get("planning.decompose", 1).unwrap();

    let mut params = BTreeMap::new();
    params.insert(
        "mission_objective".to_string(),
        "Add caching layer".to_string(),
    );
    let rendered = render_prompt(contract, &params, false).unwrap();

    assert!(
        !rendered.rendered_text.contains("EXACTLY ONE task: TASK-01"),
        "Rendered planning prompt must NOT contain artificial single-task constraint: {}",
        rendered.rendered_text
    );
    assert!(
        rendered.rendered_text.contains("0 Tasks")
            && rendered.rendered_text.contains("1 Task")
            && rendered.rendered_text.contains("N Tasks"),
        "Rendered prompt must explicitly instruct model to support 0, 1, or N tasks: {}",
        rendered.rendered_text
    );
}

// =========================================================================
// Test O: Role-specific prompt composition
// =========================================================================
#[tokio::test]
async fn test_condition_o_role_specific_prompt_composition() {
    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let compiler = ProductionContextCompiler::new().with_prompt_catalog(catalog.clone());

    let roles = [
        "execution.diagnostician",
        "execution.implementer",
        "execution.verifier",
        "runtime.safety_invariants",
    ];

    for contract_id in roles {
        let contract = catalog.get(contract_id, 1).expect("Contract missing");
        assert!(!contract.template_body.is_empty());
    }

    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 100_000)
        .with_objectives(
            Some("Implement auth middleware".to_string()),
            Some("Define session struct in src/session.rs".to_string()),
        );

    let compiled = compiler.compile_context(req).await.unwrap();

    // Must include P0 safety invariants from runtime.safety_invariants contract
    assert!(
        compiled
            .system_prompt
            .contains("M31A RUNTIME SAFETY INVARIANTS"),
        "System prompt must contain P0 safety invariants: {}",
        compiled.system_prompt
    );
    assert!(
        compiled.system_prompt.contains(".git") && compiled.system_prompt.contains(".m31a"),
        "System prompt must enumerate protected paths (.git, .m31a): {}",
        compiled.system_prompt
    );

    // Must NOT contain hardcoded parser repair cheat
    assert!(
        !compiled.system_prompt.contains("pub fn parse_expression"),
        "Hardcoded parser cheat must not exist in compiled context: {}",
        compiled.system_prompt
    );
}

// =========================================================================
// Test P: Prompt content hash / provenance
// =========================================================================
#[tokio::test]
async fn test_condition_p_prompt_content_hash_provenance() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let contracts = [
        "planning.decompose",
        "execution.diagnostician",
        "execution.verifier",
        "runtime.safety_invariants",
    ];

    for id in contracts {
        let contract = catalog
            .get(id, 1)
            .unwrap_or_else(|_| panic!("Contract {} missing", id));
        assert!(
            !contract.content_hash.is_empty(),
            "Contract {} must have content_hash",
            id
        );
        assert_eq!(
            contract.content_hash.len(),
            64,
            "content_hash must be SHA-256 hex string"
        );
        assert_eq!(contract.version, 1, "contract {} must have version 1", id);

        // Deterministic hash check: recalculating produces identical hash
        let hash2 = PromptContract::calculate_hash(
            &contract.id,
            contract.version,
            contract.role.clone(),
            &contract.description,
            &contract.input_parameters,
            &contract.template_body,
            contract.expected_output_format.as_deref(),
        );
        assert_eq!(contract.content_hash, hash2);
    }
}

// =========================================================================
// Test Q: Planner output produces a real TaskGraph
// =========================================================================
#[tokio::test]
async fn test_condition_q_planner_output_produces_real_task_graph() {
    let dir = tempdir().unwrap();
    setup_git_fixture(dir.path());
    let runtime = AppRuntime::new(dir.path()).await.unwrap();

    let plan = CandidatePlan::new(
        "PLAN-Q",
        "Multi-task execution graph",
        vec![
            CandidateTask {
                id: CandidateTaskKey::new("TASK-01"),
                objective: "Step 1: Diagnostics".to_string(),
                description: None,
                depends_on: vec![],
                capabilities: vec![CapabilityRequirement::new(
                    "fs.read",
                    CapabilityAccessMode::Read,
                )],
                role: AgentRole::diagnostician(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-02"),
                objective: "Step 2: Implementation".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-01")],
                capabilities: vec![
                    CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("fs.write", CapabilityAccessMode::Write),
                ],
                role: AgentRole::implementer(),
                verification: VerificationStrategy::Compilation,
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
            CandidateTask {
                id: CandidateTaskKey::new("TASK-03"),
                objective: "Step 3: Verification".to_string(),
                description: None,
                depends_on: vec![CandidateTaskKey::new("TASK-02")],
                capabilities: vec![
                    CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                    CapabilityRequirement::new("cargo.test", CapabilityAccessMode::ReadWrite),
                ],
                role: AgentRole::verifier(),
                verification: VerificationStrategy::AutomatedTest { command: None },
                estimates: ResourceEstimate::default(),
                ..Default::default()
            },
        ],
    );

    let mission_repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
        runtime.pool().clone(),
    );
    let mission_id = MissionId::new();
    let mission = m31a::state::mission::Mission::new(mission_id, "Test mission Q".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let materializer = TaskGraphMaterializer::new(runtime.pool().clone());
    let graph = materializer
        .materialize(mission_id, &plan)
        .await
        .expect("materialize candidate plan into authoritative TaskGraph failed");

    assert_eq!(graph.plan_id, "PLAN-Q");
    assert_eq!(graph.tasks.len(), 3);
    assert_eq!(graph.wave_tiers.len(), 3);
    assert_eq!(graph.edges.len(), 2);
}

// =========================================================================
// Production Path Scenario: Real multi-task execution through production spine
// =========================================================================
#[tokio::test]
async fn test_production_path_multi_task_execution() {
    let dir = tempdir().unwrap();
    let workspace = dir.path();

    // 1. Setup fixture repo with Cargo.toml, src/lib.rs, and tests/calc_test.rs
    setup_git_fixture(workspace);

    // Add a test that calls calculate_product(a, b) from crate::math_helper
    std::fs::create_dir_all(workspace.join("tests")).unwrap();
    std::fs::write(
        workspace.join("tests/calc_test.rs"),
        r#"use fixture_package::calculate_product;

#[test]
fn test_product() {
    assert_eq!(calculate_product(6, 7), 42);
}
"#,
    )
    .unwrap();

    // Commit the test so the workspace is in git tracking
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(workspace)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Add failing calc test"])
        .current_dir(workspace)
        .status()
        .expect("git commit failed");

    // Verify that cargo test initially fails because calculate_product doesn't exist
    let initial_test = Command::new("cargo")
        .args(["test", "--test", "calc_test"])
        .current_dir(workspace)
        .output()
        .expect("cargo test failed");
    assert!(
        !initial_test.status.success(),
        "Initial test must fail before repair"
    );

    // 2. Multi-turn autonomous model that:
    // - For Step 1 (math_helper): writes src/math_helper.rs and re-exports in src/lib.rs
    // - For Step 2 (verify): runs cargo test and confirms 42
    let model = Arc::new(MultiTaskProductionModel::new(workspace.to_path_buf()));

    let runtime = AppRuntime::new(workspace)
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    // Multi-task workflow: Step 1 (Implement helper) -> Step 2 (Run verification)
    let step1 = WorkflowStepDefinition {
        key: "step_helper".to_string(),
        name: "Implement math_helper module".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("fs.write", CapabilityAccessMode::Write),
        ],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 120,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let step2 = WorkflowStepDefinition {
        key: "step_verify".to_string(),
        name: "Verify test suite passes".to_string(),
        role: AgentRole::verifier(),
        prompt_template: "verify".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("cargo.test", CapabilityAccessMode::ReadWrite),
        ],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_helper".to_string()],
        timeout_secs: 120,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let compiled = build_test_workflow(
        "multi_task_production_wf",
        vec![step1, step2],
        RecoveryStrategy::Fail,
    );

    let start_req = WorkflowStartRequest::new(workspace).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    // Verify that both steps ran and completed through the real execution spine
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(
        run.status,
        WorkflowRunState::Completed,
        "Workflow run must complete successfully: state={:?}",
        run.status
    );

    // Verify that src/math_helper.rs now exists and cargo test calc_test passes
    assert!(workspace.join("src/math_helper.rs").exists());
    let test_output = Command::new("cargo")
        .args(["test", "--test", "calc_test"])
        .current_dir(workspace)
        .output()
        .expect("cargo test failed");
    assert!(
        test_output.status.success(),
        "cargo test must pass after multi-task execution: stdout={}",
        String::from_utf8_lossy(&test_output.stdout)
    );
}

/// Scripted model for the multi-task production path test
struct MultiTaskProductionModel {
    _workspace: PathBuf,
    turn: AtomicUsize,
}

impl MultiTaskProductionModel {
    fn new(workspace: PathBuf) -> Self {
        Self {
            _workspace: workspace,
            turn: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ModelCaller for MultiTaskProductionModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: "Initial plan ready".to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let t = self.turn.fetch_add(1, Ordering::SeqCst);

        // Check which step is executing by inspecting compiled system prompt / messages
        let is_step_helper = compiled.system_prompt.contains("Implement math_helper")
            || compiled.messages.iter().any(|m| match m {
                ChatMessage::User { content, .. } => {
                    content.contains("Implement math_helper") || content.contains("step_helper")
                }
                ChatMessage::System { content, .. } => {
                    content.contains("Implement math_helper") || content.contains("step_helper")
                }
                _ => false,
            });

        if is_step_helper {
            match t {
                0 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "write_file",
                        serde_json::json!({
                            "path": "src/math_helper.rs",
                            "content": "pub fn calculate_product(a: i32, b: i32) -> i32 { a * b }\n"
                        }),
                    )],
                }),
                1 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "write_file",
                        serde_json::json!({
                            "path": "src/lib.rs",
                            "content": "pub mod math_helper;\npub use math_helper::calculate_product;\n"
                        }),
                    )],
                }),
                2 => Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new("run_tests", serde_json::json!({}))],
                }),
                _ => Ok(ModelProposal::Complete {
                    summary:
                        "Implemented math_helper module and exported calculate_product in src/lib.rs"
                            .to_string(),
                    artifacts: vec![],
                }),
            }
        } else {
            // Step 2: Verification step
            Ok(ModelProposal::Complete {
                summary: "Verified all unit and integration tests pass".to_string(),
                artifacts: vec![],
            })
        }
    }
}
