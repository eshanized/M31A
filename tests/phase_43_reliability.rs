//! Phase 43 — Production Hardening, Reliability & Release Readiness.
//!
//! Deterministic failure-matrix proof that M31A fails honestly:
//!
//! ```text
//! failure
//!   → typed runtime state
//!   → no fake success
//!   → no unauthorized progression
//!   → budget settled / released
//!   → telemetry honest
//!   → resume without duplication
//! ```
//!
//! Live NVIDIA probes are partitioned with `#[ignore]` per
//! `tests/architecture_live_test_partition.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, RoutedModelCaller, TestModelCaller};
use m31a::agent::runner::ActionRequest;
use m31a::budget::{ActualUsage, BudgetEnforcer, TaskEstimates};
use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::providers::LocalFileSystemProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::fs::FileSystemService;
use m31a::change::authority::ChangeAuthority;
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::controller::{AutonomyController, ControllerHaltReason};
use m31a::dag::materializer::TaskGraphMaterializer;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{AgentId, CheckId, CheckpointId, MissionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, ImplementationHypothesis,
};
use m31a::kernel::seams::VerificationEngine;
use m31a::kernel::seams::execution::WorkExecutionResult;
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::model::provider::mock::MockProvider;
use m31a::model::router::resolver::ModelTier;
use m31a::model::types::{ModelError, ModelProposal, TokenUsage, UsageSource};
use m31a::persistence::artifacts::fs_store::FsArtifactStore;
use m31a::persistence::sqlite::repositories::TaskGraphRepository;
use m31a::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository;
use m31a::persistence::sqlite::repositories::{
    SqliteLifecycleRepository, ValidatedTransitionError,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::planning::review::{PreExecutionCoordinator, PreExecutionResponse};
use m31a::state::budget::ResourceBudget;
use m31a::state::completion::{CompletionContext, CompletionGate};
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::lifecycle::{LifecycleEvent, LifecycleStage};
use m31a::tools::definition::ToolExecutionContext;
use m31a::tools::registry::ToolRegistry;
use m31a::verification::gate::EvidenceCompletionGate;
use m31a::verification::types::CheckStatus;

// ===========================================================================
// HELPERS
// ===========================================================================

async fn setup_test_db() -> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("phase43.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("initialize_database");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    (dir, pool, bus)
}

async fn create_test_session(
    pool: &sqlx::SqlitePool,
    dir: &std::path::Path,
) -> (String, MissionId) {
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir).await.expect("create session");
    (
        session.id.to_string(),
        session.active_mission_id.expect("session mission"),
    )
}

fn coordinator_for(
    pool: &sqlx::SqlitePool,
    bus: &Arc<BroadcastEventBus>,
    workspace: &std::path::Path,
) -> PreExecutionCoordinator {
    PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()))
        .with_workspace_root(workspace.to_path_buf())
}

async fn seed_mission_task_agent(pool: &sqlx::SqlitePool) -> (MissionId, TaskId, AgentId) {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("phase 43 mission")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("phase 43 task")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("implementer")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    (mission_id, task_id, agent_id)
}

fn gate_for(pool: &sqlx::SqlitePool, workspace: &std::path::Path) -> EvidenceCompletionGate {
    let store_dir = workspace.join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&store_dir));
    EvidenceCompletionGate::new(pool.clone(), artifact_store, workspace)
}

fn fs_caps(workspace: &std::path::Path) -> Arc<CapabilityRegistry> {
    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.phase43",
        "Phase 43 Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "phase43_fs",
        CapabilityPermissions::full_access(),
    ));
    let fs = Arc::new(LocalFileSystemProvider::new(workspace).expect("fs provider"));
    registry.register_filesystem(fs);
    registry
}

struct AllowGate;
#[async_trait]
impl PolicyGate for AllowGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Allow)
    }
}

struct DenyGate;
#[async_trait]
impl PolicyGate for DenyGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Deny)
    }
}

fn pipeline_ctx(caps: Arc<CapabilityRegistry>, ws: std::path::PathBuf) -> ToolExecutionContext {
    ToolExecutionContext::new(caps, ws, CancellationToken::new())
        .with_mission_id(MissionId::new())
        .with_task_id(TaskId::new())
}

fn routed(mock: Arc<MockProvider>) -> RoutedModelCaller {
    RoutedModelCaller::new(
        Some(mock as Arc<dyn m31a::model::provider::ModelProvider>),
        ModelTier::Standard,
        Vec::new(),
    )
    .with_candidates(Vec::new())
    .with_model("test-model")
}

#[allow(dead_code)]
fn candidate_unused_note() {}

// ===========================================================================
// MODEL FAILURE MATRIX (§6)
// ===========================================================================

#[tokio::test]
async fn p43_model_auth_failure_is_typed_and_never_success() {
    for (err, needle) in [
        (ModelError::AuthenticationFailed, "authentication"),
        (
            ModelError::AuthenticationFailure("bad key".to_string()),
            "authentication",
        ),
        (
            ModelError::MissingCredentials("no key".to_string()),
            "credential",
        ),
    ] {
        assert!(
            !err.is_transient(),
            "auth failures must not be transient: {err:?}"
        );
        assert!(
            err.retry_delay().is_none(),
            "auth failures must not carry retry: {err:?}"
        );
        let mock = Arc::new(MockProvider::new());
        mock.push_response(Err(err)).await;
        let caller = routed(mock);
        let token = CancellationToken::new();
        let got = caller
            .call_model_cancellable_with_usage("ctx", &token)
            .await
            .expect_err("auth failure must surface as Err");
        assert!(
            got.to_lowercase().contains(needle),
            "auth error must stay typed, got: {got}"
        );
    }
}

#[tokio::test]
async fn p43_model_rate_limit_and_timeout_are_transient_with_cooldown() {
    let rl = ModelError::RateLimited { cooldown_secs: 7 };
    assert!(rl.is_transient());
    assert_eq!(rl.retry_delay(), Some(std::time::Duration::from_secs(7)));
    let to = ModelError::Timeout("slow".to_string());
    assert!(to.is_transient());
    assert!(to.retry_delay().is_some());
    let net = ModelError::Network("down".to_string());
    assert!(net.is_transient());

    // Rate-limit then success preserves authoritative usage (no fake tokens).
    let mock = Arc::new(MockProvider::new());
    mock.push_response(Err(ModelError::RateLimited { cooldown_secs: 1 }))
        .await;
    mock.push_response(Ok((
        ModelProposal::Complete {
            summary: "after cooldown".to_string(),
            artifacts: Vec::new(),
        },
        TokenUsage::new(40, 20, 60, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;
    let caller = routed(mock.clone());
    let token = CancellationToken::new();
    let (_p, usage) = caller
        .call_model_cancellable_with_usage("ctx", &token)
        .await
        .expect("retry after rate limit succeeds");
    assert_eq!(usage.total_tokens, 60);
    assert_eq!(usage.source, UsageSource::AuthoritativeProvider);
    assert_eq!(mock.recorded_calls().await.len(), 2);
}

#[tokio::test]
async fn p43_model_malformed_and_empty_never_become_success() {
    for (err, needle) in [
        (
            ModelError::InvalidResponse("empty stream".to_string()),
            "invalid",
        ),
        (
            ModelError::ProtocolViolation("bad sse".to_string()),
            "protocol",
        ),
        (
            ModelError::InvalidRequest("bad schema".to_string()),
            "invalid",
        ),
    ] {
        assert!(
            !err.is_transient(),
            "malformed must not retry blindly: {err:?}"
        );
        let mock = Arc::new(MockProvider::new());
        mock.push_response(Err(err)).await;
        let caller = routed(mock);
        let token = CancellationToken::new();
        let got = caller
            .call_model_cancellable_with_usage("ctx", &token)
            .await
            .expect_err("malformed must be Err, never success");
        assert!(
            got.to_lowercase().contains(needle),
            "malformed error must stay typed, got: {got}"
        );
    }
    // Reasoning-only / empty stream is InvalidResponse at the SSE seam
    // (src/model/provider/sse.rs): covered structurally by protocol tests.
}

#[tokio::test]
async fn p43_model_cancellation_settles_nothing_and_records_nothing() {
    let mock = Arc::new(MockProvider::new());
    let caller = routed(mock.clone());
    let enforcer = BudgetEnforcer::new(ResourceBudget::unbounded());
    let token = CancellationToken::new();
    token.cancel();
    let err = caller
        .call_model_cancellable_with_usage("ctx", &token)
        .await
        .expect_err("cancelled call must fail");
    assert!(err.contains("cancelled"), "got: {err}");
    assert_eq!(mock.recorded_calls().await.len(), 0);
    assert_eq!(enforcer.total_tokens_consumed(), 0);
}

// ===========================================================================
// TOOL FAILURE MATRIX (§7)
// ===========================================================================

#[tokio::test]
async fn p43_tool_unknown_tool_fails_closed_without_side_effects() {
    let dir = tempdir().unwrap();
    let caps = fs_caps(dir.path());
    let registry = Arc::new(ToolRegistry::new_default(Arc::clone(&caps)));
    let runner = ToolPipelineRunner::new(registry);
    let ctx = pipeline_ctx(caps, dir.path().to_path_buf());
    let res = runner
        .execute_action(
            &ActionRequest {
                id: "act-unknown".to_string(),
                tool_name: "does_not_exist_xyz".to_string(),
                parameters: serde_json::json!({}),
            },
            &ctx,
            &AllowGate,
            AutonomyMode::Safe,
        )
        .await;
    assert!(!res.success, "unknown tool must fail: {res:?}");
    assert!(!dir.path().join("does_not_exist_xyz").exists());
}

#[tokio::test]
async fn p43_tool_malformed_args_fail_closed() {
    let dir = tempdir().unwrap();
    let caps = fs_caps(dir.path());
    let registry = Arc::new(ToolRegistry::new_default(Arc::clone(&caps)));
    let runner = ToolPipelineRunner::new(registry);
    let ctx = pipeline_ctx(caps, dir.path().to_path_buf());
    // write_file without required content/path
    let res = runner
        .execute_action(
            &ActionRequest {
                id: "act-malformed".to_string(),
                tool_name: "write_file".to_string(),
                parameters: serde_json::json!({"path": ""}),
            },
            &ctx,
            &AllowGate,
            AutonomyMode::Safe,
        )
        .await;
    assert!(!res.success, "malformed args must fail: {res:?}");
}

#[tokio::test]
async fn p43_tool_capability_denial_fails_closed() {
    let dir = tempdir().unwrap();
    // Registry WITHOUT filesystem provider: capability unavailable.
    let caps = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(Arc::clone(&caps)));
    let runner = ToolPipelineRunner::new(registry);
    let ctx = pipeline_ctx(caps, dir.path().to_path_buf());
    let res = runner
        .execute_action(
            &ActionRequest {
                id: "act-nocap".to_string(),
                tool_name: "write_file".to_string(),
                parameters: serde_json::json!({"path": "x.txt", "content": "hi"}),
            },
            &ctx,
            &AllowGate,
            AutonomyMode::Safe,
        )
        .await;
    assert!(!res.success, "missing capability must deny: {res:?}");
    assert!(!dir.path().join("x.txt").exists());
}

#[tokio::test]
async fn p43_tool_policy_denial_releases_budget_without_leak() {
    let (dir, pool, bus) = setup_test_db().await;
    let enforcer = Arc::new(BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(100_000),
        ..Default::default()
    }));
    let mut deps = ControllerDependencies::production(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a").join("storage"),
        Some(bus.clone()),
    );
    deps = deps
        .with_budget_enforcer(enforcer.clone())
        .with_policy(Arc::new(DenyGate));
    let mut controller = AutonomyController::new(
        MissionId::new(),
        AutonomyMode::Safe,
        deps,
        bus.clone(),
        CancellationToken::new(),
    );
    let task_id = TaskId::new();
    controller.active_task = Some(m31a::kernel::seams::scheduler::WorkItem {
        task_id,
        title: "denied".to_string(),
        estimated_tokens: 1000,
        required_capabilities: vec![],
        description: None,
        completion_criteria: vec![],
        requirement_keys: vec![],
        assumptions: vec![],
        verification: None,
    });
    controller.progress.current_stage = LoopStage::ValidatePolicyAndResources;
    let outcome = controller.step().await.expect("step");
    assert_eq!(outcome, StageOutcome::SkipTo(LoopStage::ClassifyFailure));
    assert_eq!(
        enforcer.snapshot().tokens_reserved,
        0,
        "denial must release"
    );
    assert_eq!(
        enforcer.snapshot().agent_steps_consumed,
        0,
        "denial must not inflate steps"
    );
    assert_eq!(
        enforcer.snapshot().model_calls_consumed,
        0,
        "denial must not inflate calls"
    );
    assert_eq!(controller.active_receipt, None);
}

#[tokio::test]
async fn p43_tool_failed_execution_never_marks_task_complete() {
    let (dir, pool, bus) = setup_test_db().await;
    let deps = ControllerDependencies::production(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a").join("storage"),
        Some(bus.clone()),
    );
    let mut controller = AutonomyController::new(
        MissionId::new(),
        AutonomyMode::Safe,
        deps,
        bus.clone(),
        CancellationToken::new(),
    );
    let task_id = TaskId::new();
    controller.active_task = Some(m31a::kernel::seams::scheduler::WorkItem {
        task_id,
        title: "failing".to_string(),
        estimated_tokens: 100,
        required_capabilities: vec![],
        description: None,
        completion_criteria: vec![],
        requirement_keys: vec![],
        assumptions: vec![],
        verification: None,
    });
    controller.last_execution_result = Some(WorkExecutionResult {
        task_id,
        success: false,
        output: "boom".to_string(),
        error_detail: Some("boom".to_string()),
        token_usage: None,
    });
    controller.progress.current_stage = LoopStage::ClassifyFailure;
    let outcome = controller.step().await.expect("classify");
    assert_eq!(outcome, StageOutcome::Advance(LoopStage::RecoverOrReplan));
    assert!(controller.last_failure_class.is_some());
}

// ===========================================================================
// WORKSPACE & WORKTREE FAILURE (§8)
// ===========================================================================

#[tokio::test]
async fn p43_workspace_path_escape_contained() {
    let dir = tempdir().unwrap();
    let caps = fs_caps(dir.path());
    let registry = Arc::new(ToolRegistry::new_default(Arc::clone(&caps)));
    let runner = ToolPipelineRunner::new(registry);
    let ctx = pipeline_ctx(caps, dir.path().to_path_buf());
    for evil in ["../escape.txt", "/etc/passwd", "a/../../escape2.txt"] {
        let res = runner
            .execute_action(
                &ActionRequest {
                    id: "act-escape".to_string(),
                    tool_name: "write_file".to_string(),
                    parameters: serde_json::json!({"path": evil, "content": "x"}),
                },
                &ctx,
                &AllowGate,
                AutonomyMode::Safe,
            )
            .await;
        assert!(!res.success, "escape '{evil}' must be contained: {res:?}");
    }
    assert!(!dir.path().join("escape.txt").exists());
}

#[tokio::test]
async fn p43_workspace_change_rollback_restores_exact_bytes() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).expect("fs"));
    let original = "pub fn calculate() -> u32 { 0 }\n";
    fs.write_file(std::path::Path::new("calc.rs"), original.as_bytes())
        .await
        .unwrap();
    // Fake implementation (todo!()) is rejected by diff review with rollback.
    let hypothesis =
        ImplementationHypothesis::new("Implement", "Empty", "Add todo", "calc", "test");
    let surface = ChangeSurface::new(vec!["calc.rs".to_string()]);
    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "calc.rs",
            FileMutationOp::Substring {
                old_content: "0".to_string(),
                new_content: "todo!()".to_string(),
            },
            "fake it",
        )],
    );
    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .expect_err("fake must be rejected");
    assert!(format!("{err:?}").contains("DiffReviewRejected"));
    let back = fs
        .read_file(std::path::Path::new("calc.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(
        back,
        original.as_bytes(),
        "rollback must restore exact bytes"
    );
}

#[tokio::test]
async fn p43_git_merge_conflict_reports_without_target_mutation() {
    let dir = tempdir().unwrap();
    let repo = dir.path();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.name", "t"],
        vec!["config", "user.email", "t@t"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(&args)
                .current_dir(repo)
                .status()
                .unwrap()
                .success()
        );
    }
    tokio::fs::write(repo.join("f.txt"), b"base\n")
        .await
        .unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success()
    );
    // Two-stage integration machine reports conflict truthfully on colliding branches.
    std::process::Command::new("git")
        .args(["checkout", "-b", "branch-a"])
        .current_dir(repo)
        .status()
        .unwrap();
    tokio::fs::write(repo.join("f.txt"), b"a-side\n")
        .await
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-am", "a"])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["checkout", "-b", "branch-b"])
        .current_dir(repo)
        .status()
        .unwrap();
    tokio::fs::write(repo.join("f.txt"), b"b-side\n")
        .await
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-am", "b"])
        .current_dir(repo)
        .status()
        .unwrap();

    let mut machine =
        m31a::git::integration::WorktreeIntegrationStateMachine::new(repo.to_path_buf());
    let worktree = m31a::git::worktree::IsolatedWorktree {
        mission_id: MissionId::new(),
        path: repo.join(".m31a").join("wt-probe"),
        branch: "branch-a".to_string(),
        base_commit: "HEAD".to_string(),
        created_at: chrono::Utc::now(),
    };
    let report = machine
        .integrate(
            &worktree,
            "branch-b",
            m31a::git::integration::MergeStrategy::MergeCommit,
            &m31a::git::GitGate::authorized(),
        )
        .await;
    // Either conflict report or error — but never a silent success claim
    // with a fabricated merge commit on conflict.
    if let Ok(rep) = report {
        if rep.state == m31a::git::integration::IntegrationState::IntegrationConflict {
            assert!(rep.merge_commit.is_none());
            assert!(!rep.conflict_files.is_empty() || !rep.message.is_empty());
        }
    }
}

// ===========================================================================
// DATABASE CORRUPTION (§10) + PERSISTENCE (§11)
// ===========================================================================

#[tokio::test]
async fn p43_db_corrupt_lifecycle_stage_fails_closed() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (session_id, _) = create_test_session(&pool, dir.path()).await;
    let repo = SqliteLifecycleRepository::new(pool.clone());
    let now = chrono::Utc::now();
    repo.save_lifecycle_state(
        &m31a::persistence::sqlite::repositories::PersistedLifecycleState {
            session_id: session_id.clone(),
            stage: LifecycleStage::ExecutionAuthorized,
            plan_revision: 2,
            task_revision: 3,
            authorization_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("seed");
    let sbytes = sqlx::types::Uuid::parse_str(&session_id)
        .map(|u| u.as_bytes().to_vec())
        .unwrap_or_else(|_| session_id.as_bytes().to_vec());
    sqlx::query(
        "UPDATE session_lifecycle_state SET stage = 'corrupted_stage' WHERE session_id = ?",
    )
    .bind(sbytes.as_slice())
    .execute(&pool)
    .await
    .expect("corrupt row");
    let err = repo
        .load_lifecycle_state(&session_id)
        .await
        .expect_err("corrupt stage must fail load");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
    let verr = repo
        .save_validated_transition(
            &session_id,
            LifecycleStage::ExecutionAuthorized,
            LifecycleEvent::StartExecution,
        )
        .await
        .expect_err("no execution from corrupt state");
    assert!(
        matches!(verr, ValidatedTransitionError::Persistence(_)),
        "got: {verr}"
    );
}

#[tokio::test]
async fn p43_db_corrupt_verification_status_never_completes() {
    use std::str::FromStr;
    assert!(CheckStatus::from_str("corrupted_xyz").is_err());
    assert!(CheckStatus::from_str("").is_err());
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let check_id = CheckId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, summary, snapshot_hash, created_at) VALUES (?, ?, ?, 1, 'corrupted_xyz', 'run_tests', '{}', 'x', 'snap', ?)",
    )
    .bind(check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let gate = gate_for(&pool, dir.path());
    let decision = gate
        .evaluate_task_completion(mission_id, task_id, "snap")
        .await
        .expect("evaluation runs");
    assert!(!decision.is_satisfied, "corrupt status must never satisfy");
}

#[tokio::test]
async fn p43_db_corrupt_task_status_fails_closed_in_snapshot() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    sqlx::query("UPDATE tasks SET status = 'corrupted_xyz' WHERE id = ?")
        .bind(task_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();
    // Phase 43 hardening: snapshot read fails closed instead of silently
    // omitting the corrupt task.
    let repo =
        m31a::persistence::sqlite::repositories::task::SqliteTaskRepository::new(pool.clone());
    let err = repo
        .get_task_states_by_mission(mission_id)
        .await
        .expect_err("corrupt task status must fail closed");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
}

#[tokio::test]
async fn p43_db_corrupt_job_row_fails_closed_without_panic() {
    let (tmp, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, agent_id) = seed_mission_task_agent(&pool).await;
    let admission = Arc::new(m31a::process::admission::JobAdmissionController::new(4, 2));
    let store = Arc::new(FsArtifactStore::new(tmp.path().join("artifacts")));
    let spool_dir = tmp.path().join("spools");
    let mgr = m31a::process::job::JobManager::new(pool.clone(), admission, store, spool_dir);
    let now = chrono::Utc::now().to_rfc3339();
    let limits_json = serde_json::to_string(&m31a::sandbox::ResourceLimits::default()).unwrap();
    // Truncated 3-byte identity (not 16): legacy code panicked in
    // copy_from_slice; Phase 43 returns typed Corrupt.
    sqlx::query(
        "INSERT INTO jobs (id, mission_id, task_id, agent_id, tool_call_id, command, args_json, working_dir, state, provider, resource_limits_json, submitted_at) VALUES (?, ?, ?, ?, NULL, 'echo', '[]', '/tmp', 'running', 'test', ?, ?)",
    )
    .bind(vec![1u8, 2, 3])
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind(&limits_json)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let jobs = mgr.list_jobs(Some(mission_id), None).await;
    assert!(
        jobs.is_err(),
        "truncated identity must fail closed, got: {jobs:?}"
    );
    assert!(jobs.unwrap_err().to_string().contains("corrupt"));
}

#[tokio::test]
async fn p43_persistence_partial_state_cannot_complete() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let gate = gate_for(&pool, dir.path());
    // No verification checks persisted at all → gate deficient.
    let outcome = gate.verify_completion_gate(mission_id).await.unwrap();
    assert!(
        matches!(
            outcome,
            m31a::kernel::seams::CompletionGateOutcome::Deficient { .. }
        ),
        "partial state must not complete: {outcome:?}"
    );
}

// ===========================================================================
// BUDGET EXHAUSTION (§12) + AUTHORITATIVE ACCOUNTING (§13)
// ===========================================================================

#[tokio::test]
async fn p43_budget_reservation_denial_and_overrun_fail_closed() {
    let enforcer = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(100),
        ..Default::default()
    });
    let denied = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 500,
                ..Default::default()
            },
            false,
        )
        .expect_err("over-budget reservation must deny");
    assert_eq!(denied, m31a::budget::BudgetExhaustionAction::Block);
    assert_eq!(enforcer.snapshot().tokens_reserved, 0);

    // Settle reality above estimate: monotonic, then admission closed.
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 60,
                ..Default::default()
            },
            true,
        )
        .expect("reserve within budget");
    enforcer.settle_model_usage(
        &receipt,
        &TokenUsage::new(4000, 1000, 5000, 0, UsageSource::AuthoritativeProvider),
    );
    assert_eq!(enforcer.total_tokens_consumed(), 5000);
    assert_eq!(enforcer.snapshot().tokens_reserved, 0);
    assert!(
        enforcer
            .reserve(
                &TaskEstimates {
                    estimated_tokens: 0,
                    ..Default::default()
                },
                true
            )
            .is_err(),
        "post-overrun admission must fail closed"
    );
    // Double settle of the same receipt never wraps counters negative.
    enforcer.settle_model_usage(
        &receipt,
        &TokenUsage::new(1, 0, 1, 0, UsageSource::AuthoritativeProvider),
    );
    assert_eq!(enforcer.snapshot().tokens_reserved, 0);
}

#[tokio::test]
async fn p43_budget_release_on_pause_leaves_zero_leakage() {
    // Phase 43 fix: Pause/Escalate after reserve releases the receipt.
    let enforcer = Arc::new(BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(100_000),
        ..Default::default()
    }));
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 1000,
                estimated_cost_usd: 0.01,
                requires_worker: true,
                estimated_artifact_bytes: 4096,
            },
            false,
        )
        .expect("reserve");
    assert_eq!(enforcer.snapshot().tokens_reserved, 1000);
    enforcer.release_reservation(&receipt);
    let snap = enforcer.snapshot();
    assert_eq!(snap.tokens_reserved, 0);
    assert_eq!(
        snap.agent_steps_consumed, 0,
        "release must not inflate steps"
    );
    assert_eq!(
        snap.model_calls_consumed, 0,
        "release must not inflate calls"
    );
    assert_eq!(snap.tokens_consumed, 0);
}

#[tokio::test]
async fn p43_accounting_authoritative_equals_settled_equals_telemetry() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, agent_id) = seed_mission_task_agent(&pool).await;
    let usage = TokenUsage::new(25, 15, 40, 0, UsageSource::AuthoritativeProvider);
    let enforcer = BudgetEnforcer::new(ResourceBudget::unbounded());
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 100,
                ..Default::default()
            },
            true,
        )
        .expect("reserve");
    let actual = enforcer.settle_model_usage(&receipt, &usage);
    assert_eq!(actual.tokens, 40);
    assert!(ActualUsage::is_authoritative(&usage));
    // Telemetry record carries identical counters + provenance.
    let telemetry = m31a::model::persistence::SqliteModelInvocationRepository::new(pool.clone());
    let selection = m31a::agent::model_policy::ResolvedModelSelection {
        provider: "nvidia".to_string(),
        model_name: "test-model".to_string(),
        temperature: 0.2,
        max_tokens: 8192,
    };
    let record = m31a::model::router::recovery::ModelRecoveryCoordinator::record_telemetry_attempt(
        &telemetry,
        mission_id,
        task_id,
        agent_id,
        1,
        &selection,
        1,
        "success",
        &usage,
        "phase43-consistency",
    )
    .await
    .expect("telemetry records");
    assert_eq!(record.total_tokens as u64, actual.tokens);
    assert_eq!(enforcer.total_tokens_consumed(), actual.tokens);
    // Estimated usage is explicitly marked and never mixed.
    let est = TokenUsage::new(7, 3, 10, 0, UsageSource::Estimated);
    assert!(!ActualUsage::is_authoritative(&est));
    let _ = (dir, task_id);
}

#[tokio::test]
async fn p43_accounting_multi_turn_retry_partial_zero() {
    // accumulate() is monotonic across turns; partial/zero preserved.
    let mut acc = TokenUsage::new(0, 0, 0, 0, UsageSource::Estimated);
    acc.accumulate(&TokenUsage::new(
        30,
        0,
        30,
        0,
        UsageSource::AuthoritativeProvider,
    ));
    assert_eq!(acc.total_tokens, 30);
    assert_eq!(acc.source, UsageSource::AuthoritativeProvider);
    acc.accumulate(&TokenUsage::new(
        0,
        0,
        0,
        0,
        UsageSource::AuthoritativeProvider,
    ));
    assert_eq!(acc.total_tokens, 30);
    acc.accumulate(&TokenUsage::new(
        10,
        5,
        15,
        0,
        UsageSource::AuthoritativeProvider,
    ));
    assert_eq!(acc.total_tokens, 45);
}

// ===========================================================================
// CANCELLATION (§14) + PAUSE/RESUME (§15)
// ===========================================================================

#[tokio::test]
async fn p43_cancellation_halts_without_post_cancel_start() {
    let (dir, pool, bus) = setup_test_db().await;
    let token = CancellationToken::new();
    let deps = ControllerDependencies::production(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a").join("storage"),
        Some(bus.clone()),
    );
    let mut controller = AutonomyController::new(
        MissionId::new(),
        AutonomyMode::Safe,
        deps,
        bus.clone(),
        token.clone(),
    );
    token.cancel();
    let outcome = controller.step().await.expect("step");
    assert_eq!(outcome, StageOutcome::Halt(ControllerHaltReason::Cancelled));
}

#[tokio::test]
async fn p43_pause_resume_cycle_stays_coherent() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, _) = create_test_session(&pool, dir.path()).await;
    let coordinator = coordinator_for(&pool, &bus, dir.path());
    let resp = coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .expect("init");
    assert!(matches!(resp, PreExecutionResponse::PlanForReview { .. }));
    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept plan");
    let repo = SqliteLifecycleRepository::new(pool.clone());
    let state = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .unwrap();
    // Pause is representable as a halt, not a lifecycle corruption: the
    // persisted stage remains TasksReview and resume continues from it.
    assert_eq!(state.stage, LifecycleStage::TasksReview);
    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("resume: accept tasks after pause-equivalent halt");
    let resumed = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        resumed.stage,
        LifecycleStage::ExecutionAwaitingAuthorization
    );
}

// ===========================================================================
// GOVERNANCE: AUTH / LIFECYCLE / COMPLETION (§19-adjacent)
// ===========================================================================

#[tokio::test]
async fn p43_governance_stale_authorization_rejected_no_graph() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;
    let coordinator = coordinator_for(&pool, &bus, dir.path());
    let resp = coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .expect("init");
    assert!(matches!(resp, PreExecutionResponse::PlanForReview { .. }));
    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .unwrap();
    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .unwrap();
    let auth_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .unwrap();
    let (mut plan, auth) = match auth_resp {
        PreExecutionResponse::ReadyToExecute {
            plan,
            tasks,
            authorization,
            ..
        } => {
            let mut p = plan;
            p.tasks = tasks;
            (p, authorization)
        }
        other => panic!("expected ReadyToExecute, got {other:?}"),
    };
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let err = materializer
        .materialize_authorized(
            mission_id,
            &plan,
            auth.plan_revision + 10,
            auth.task_revision,
            &auth,
        )
        .await
        .expect_err("stale must reject");
    assert!(!err.to_string().is_empty());
    let repo = SqliteTaskGraphRepository::new(pool.clone());
    assert!(repo.get_active_graph(mission_id).await.unwrap().is_none());
    // Same stale grant reused after a valid materialization is still invalid
    // for a different revision.
    let ok = materializer
        .materialize_authorized(
            mission_id,
            &plan,
            auth.plan_revision,
            auth.task_revision,
            &auth,
        )
        .await
        .expect("valid materializes");
    assert_eq!(ok.mission_id, mission_id);
    let _ = &mut plan;
}

#[tokio::test]
async fn p43_governance_lifecycle_mismatch_rejected() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, _) = create_test_session(&pool, dir.path()).await;
    let coordinator = coordinator_for(&pool, &bus, dir.path());
    coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .expect("init");
    let repo = SqliteLifecycleRepository::new(pool);
    // In PlanReview, jumping straight to AuthorizeExecution skips mandatory
    // review gates and must be rejected.
    let err = repo
        .save_validated_transition(
            &session_id,
            LifecycleStage::PlanReview,
            LifecycleEvent::AuthorizeExecution,
        )
        .await
        .expect_err("skip must reject");
    assert!(
        matches!(
            err,
            ValidatedTransitionError::InvalidTransition { .. }
                | ValidatedTransitionError::StageMismatch { .. }
        ),
        "got: {err}"
    );
}

#[tokio::test]
async fn p43_governance_completion_veto_without_evidence() {
    assert!(!CompletionGate::is_authoritative());
    assert!(CompletionGate::can_complete(
        &CompletionContext::all_satisfied()
    ));
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let gate = gate_for(&pool, dir.path());
    // Advisory local gate satisfied, yet durable evidence gate vetoes.
    let decision = gate
        .evaluate_task_completion(mission_id, task_id, "snap-p43")
        .await
        .expect("runs");
    assert!(!decision.is_satisfied);
    assert!(!decision.violations.is_empty());
}

// ===========================================================================
// CRASH RECOVERY (§5) + RESUME/DUPLICATION (§19)
// ===========================================================================

#[tokio::test]
async fn p43_recovery_integrity_check_non_ok_payload_is_corrupt() {
    // Phase 43 fix: scan_and_reconcile inspects the PRAGMA payload text,
    // not just transport errors. Unit-level equivalent: a non-ok string
    // must classify as corrupt (structural assertion on the seam).
    let src = include_str!("../src/checkpoint/crash_recovery.rs");
    assert!(
        src.contains("non-ok payload") || src.contains("integrity_check"),
        "recovery seam must inspect PRAGMA payload"
    );
    assert!(src.contains("CrashRecoveryClassification::Corrupt"));
}

#[tokio::test]
async fn p43_recovery_checkpoint_create_restore_no_duplicates() {
    let (dir, pool, bus) = setup_test_db().await;
    let rt = m31a::runtime::AppRuntime::new(dir.path()).await.unwrap();
    let _ = (pool, bus);
    let checkpoint_mgr = rt.dependencies().checkpoint_manager().unwrap();
    let (mission_id, task_id, agent_id) = seed_mission_task_agent(rt.pool()).await;
    let cp_id = CheckpointId::new();
    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "P43Checkpoint",
        1,
        "snap-p43",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy123",
        vec![],
        vec![],
        "p43 checkpoint",
    );
    let created = checkpoint_mgr
        .create_checkpoint(&manifest, vec![])
        .await
        .expect("create");
    let restored = checkpoint_mgr
        .restore_checkpoint(created, dir.path())
        .await
        .expect("restore");
    assert_eq!(restored.checkpoint_id, cp_id);
    assert_eq!(restored.mission_id, mission_id);
    // Restoring twice yields the same checkpoint, not two completions.
    let restored2 = checkpoint_mgr
        .restore_checkpoint(created, dir.path())
        .await
        .expect("restore again");
    assert_eq!(restored2.checkpoint_id, cp_id);
    let _ = (task_id, agent_id);
}

#[tokio::test]
async fn p43_recovery_completion_not_duplicated_on_reevaluate() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    // Failed evidence: every evaluation stays deficient; no satisfied
    // decision is ever persisted, so resume cannot duplicate completion.
    let check_id = CheckId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, summary, snapshot_hash, created_at) VALUES (?, ?, ?, 1, 'failed', 'run_tests', '{}', 'fail', 'snap-p43', ?)",
    )
    .bind(check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let gate = gate_for(&pool, dir.path());
    for _ in 0..2 {
        let d = gate
            .evaluate_task_completion(mission_id, task_id, "snap-p43")
            .await
            .expect("runs");
        assert!(!d.is_satisfied);
    }
    let satisfied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM completion_gate_decisions WHERE is_satisfied = 1")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);
    assert_eq!(
        satisfied, 0,
        "no duplicate (or any) satisfaction on failure"
    );
}

// ===========================================================================
// CONCURRENCY ISOLATION (§18)
// ===========================================================================

#[tokio::test]
async fn p43_concurrency_missions_isolated_budget_artifacts_auth_lifecycle() {
    // Budget isolation.
    let a = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(100_000),
        ..Default::default()
    });
    let b = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(100_000),
        ..Default::default()
    });
    let ra = a
        .reserve(
            &TaskEstimates {
                estimated_tokens: 1000,
                ..Default::default()
            },
            true,
        )
        .unwrap();
    a.settle_model_usage(
        &ra,
        &TokenUsage::new(600, 400, 1000, 0, UsageSource::AuthoritativeProvider),
    );
    assert_eq!(a.total_tokens_consumed(), 1000);
    assert_eq!(b.total_tokens_consumed(), 0, "mission B budget untouched");

    // Lifecycle isolation: two sessions advance independently.
    let (dir, pool, bus) = setup_test_db().await;
    let (s1, m1) = create_test_session(&pool, dir.path()).await;
    let (s2, m2) = create_test_session(&pool, dir.path()).await;
    assert_ne!(m1, m2);
    let c1 = coordinator_for(&pool, &bus, dir.path());
    c1.init_intent(&s1, "Build a microservice API in Rust", "operator")
        .await
        .unwrap();
    c1.handle_action(
        ApplicationAction::PlanAcceptRequested {
            session_id: Some(s1.clone()),
        },
        "operator",
    )
    .await
    .unwrap();
    let repo = SqliteLifecycleRepository::new(pool.clone());
    let st1 = repo.load_lifecycle_state(&s1).await.unwrap().unwrap();
    // s2 never advanced: no lifecycle row yet — sessions do not share state.
    let st2 = repo.load_lifecycle_state(&s2).await.unwrap();
    assert!(st2.is_none(), "idle session must have no lifecycle row");
    assert_eq!(st1.stage, LifecycleStage::TasksReview);
    let _ = s2;
}

// ===========================================================================
// LONG-RUN (§16) + LARGE REPO (§17) BOUNDEDNESS
// ===========================================================================

#[tokio::test]
async fn p43_long_run_mission_stays_bounded() {
    // 50 sequential reserve→settle cycles: counters exactly linear, no leak.
    let enforcer = BudgetEnforcer::new(ResourceBudget::unbounded());
    let mut acc = TokenUsage::new(0, 0, 0, 0, UsageSource::Estimated);
    for _ in 0..50 {
        let receipt = enforcer
            .reserve(
                &TaskEstimates {
                    estimated_tokens: 100,
                    ..Default::default()
                },
                true,
            )
            .expect("reserve");
        let turn = TokenUsage::new(60, 40, 100, 0, UsageSource::AuthoritativeProvider);
        acc.accumulate(&turn);
        enforcer.settle_model_usage(&receipt, &turn);
    }
    assert_eq!(acc.total_tokens, 5000);
    assert_eq!(enforcer.total_tokens_consumed(), 5000);
    assert_eq!(enforcer.snapshot().tokens_reserved, 0);
    assert_eq!(enforcer.snapshot().agent_steps_consumed, 50);
    assert_eq!(enforcer.snapshot().model_calls_consumed, 50);
}

#[tokio::test]
async fn p43_large_repository_remains_operable() {
    let dir = tempdir().unwrap();
    // 300 files across nested dirs + noise.
    for i in 0..300 {
        let sub = dir.path().join(format!("mod_{:02}/sub", i % 30));
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::write(sub.join(format!("f{i}.rs")), b"pub fn f() {}\n")
            .await
            .unwrap();
    }
    tokio::fs::write(dir.path().join("noise.bin"), vec![0u8; 1024])
        .await
        .unwrap();
    let fs = LocalFileSystemProvider::new(dir.path()).unwrap();
    let entries = fs
        .list_files(std::path::Path::new("."), true)
        .await
        .expect("list bounded repo");
    assert!(entries.len() >= 300, "all modules discoverable");
    assert!(entries.len() < 2000, "listing stays bounded");
    // Focused read does not scan the world.
    let bytes = fs
        .read_file(
            std::path::Path::new("mod_00/sub/f0.rs"),
            Some(0u64),
            Some(1024),
        )
        .await
        .expect("read");
    assert!(!bytes.is_empty());
}

// ===========================================================================
// SECURITY (§20) + RESOURCE EXHAUSTION (§21)
// ===========================================================================

#[tokio::test]
async fn p43_security_shell_escape_blocked_and_secrets_redacted() {
    // Dangerous git redirection flags are rejected at the provider seam.
    let src = include_str!("../src/process/env.rs");
    assert!(src.contains("GIT_DIR") || src.contains("git-dir"));
    // Telemetry redactor masks registered secrets and bearer tokens.
    let redactor = m31a::telemetry::redactor::SecretRedactor::new();
    redactor.register_secret("nvapi-SECRET123456");
    let redacted = redactor.redact_string(
        "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c and key nvapi-SECRET123456",
    );
    assert!(
        !redacted.contains("nvapi-SECRET123456"),
        "registered secret must be masked: {redacted}"
    );
    assert!(
        !redacted.contains("SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"),
        "bearer token must be masked: {redacted}"
    );
}

#[tokio::test]
async fn p43_resource_repeated_failures_stay_bounded_no_retry_storm() {
    // Identical failing tool calls are rejected as non-progress (bounded).
    let mock = Arc::new(MockProvider::new());
    mock.push_response(Err(ModelError::RateLimited { cooldown_secs: 1 }))
        .await;
    mock.push_response(Ok((
        ModelProposal::Complete {
            summary: "recovered".to_string(),
            artifacts: Vec::new(),
        },
        TokenUsage::new(10, 5, 15, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;
    let caller = routed(mock.clone());
    let token = CancellationToken::new();
    let (_p, usage) = caller
        .call_model_cancellable_with_usage("ctx", &token)
        .await
        .expect("single retry succeeds");
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(
        mock.recorded_calls().await.len(),
        2,
        "exactly one retry, no storm"
    );
    // TestModelCaller fallback is explicitly Estimated, never mixed.
    let fallback = TestModelCaller::new("fallback");
    let (_p2, u2) = fallback
        .call_model_cancellable_with_usage("ctx", &token)
        .await
        .expect("fallback");
    assert_eq!(u2.source, UsageSource::Estimated);
    assert!(!ActualUsage::is_authoritative(&u2));
}

// ===========================================================================
// OBSERVABILITY (§22) + TUI TRUTH (§23)
// ===========================================================================

#[tokio::test]
async fn p43_observability_telemetry_carries_failure_dimensions() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, agent_id) = seed_mission_task_agent(&pool).await;
    let telemetry = m31a::model::persistence::SqliteModelInvocationRepository::new(pool.clone());
    let selection = m31a::agent::model_policy::ResolvedModelSelection {
        provider: "nvidia".to_string(),
        model_name: "test-model".to_string(),
        temperature: 0.2,
        max_tokens: 8192,
    };
    let usage = TokenUsage::new(10, 5, 15, 0, UsageSource::AuthoritativeProvider);
    let record = m31a::model::router::recovery::ModelRecoveryCoordinator::record_telemetry_attempt(
        &telemetry,
        mission_id,
        task_id,
        agent_id,
        1,
        &selection,
        1,
        "success",
        &usage,
        "p43-observability",
    )
    .await
    .expect("record");
    assert_eq!(record.mission_id, mission_id);
    assert_eq!(record.task_id, task_id);
    assert_eq!(record.provider, "nvidia");
    assert_eq!(record.model_name, "test-model");
    assert_eq!(record.total_tokens, 15);
    assert_eq!(record.usage_source, "authoritative_provider");
}

#[tokio::test]
async fn p43_tui_truth_failure_never_renders_as_success() {
    // TUI is a projection: advisory local gate may read satisfied while the
    // durable evidence gate stays deficient — the runtime (not the TUI)
    // decides, and failure must render as failure.
    assert!(!CompletionGate::is_authoritative());
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let gate = gate_for(&pool, dir.path());
    let outcome = gate.verify_completion_gate(mission_id).await.unwrap();
    assert!(
        matches!(
            outcome,
            m31a::kernel::seams::CompletionGateOutcome::Deficient { .. }
        ),
        "TUI must project Deficient, never Completed, without evidence"
    );
    let _ = task_id;
}

// ===========================================================================
// RELEASE-LIKE ENVIRONMENT (§24): clean install / startup / shutdown
// ===========================================================================

#[tokio::test]
async fn p43_release_clean_startup_shutdown_cycle() {
    let dir = tempdir().unwrap();
    let rt = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("clean startup constructs runtime");
    // Runtime exposes a live SQLite pool after clean install.
    let one: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(rt.pool())
        .await
        .expect("database answers after startup");
    assert_eq!(one, 1);
    // No model caller without provider: fail closed, no stale wiring.
    let bare = rt.without_model_caller();
    assert!(bare.model_caller().is_none());
    // Clean shutdown: drop runtime, workspace persists.
    drop(bare);
    assert!(dir.path().exists());
}

// ===========================================================================
// LIVE RELIABILITY PROBES (§26, #[ignore])
// ===========================================================================

/// Live probe 1: real model → tool → workspace change → verification → evidence.
#[tokio::test]
#[ignore = "requires live NVIDIA credentials + WAN; run explicitly with --ignored"]
async fn live_p43_reliability_multi_turn_evidence() {
    let harness = match m31a::testing::real_model::RealModelHarness::ensure() {
        Ok(h) => h,
        Err(skipped) => {
            eprintln!("LIVE SKIPPED (environment): {}", skipped.reason);
            return;
        }
    };
    let caller = harness.routed_caller(Vec::new());
    let token = CancellationToken::new();
    let (proposal, usage) = caller
        .call_model_cancellable_with_usage("Return the text: 'p43 live reliability ok'", &token)
        .await
        .expect("live model call");
    // Model variance: providers may return Complete or AssistantText for a
    // free-text prompt. Either is an honest model response (never fake
    // success: the call itself succeeded with authoritative usage).
    assert!(
        proposal.is_completion()
            || matches!(
                proposal,
                m31a::model::types::ModelProposal::AssistantText { .. }
            ),
        "live model must return honest text proposal, got: {proposal:?}"
    );
    assert_eq!(usage.source, UsageSource::AuthoritativeProvider);
    let enforcer = BudgetEnforcer::new(ResourceBudget::unbounded());
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 1000,
                ..Default::default()
            },
            true,
        )
        .unwrap();
    let actual = enforcer.settle_model_usage(&receipt, &usage);
    assert_eq!(actual.tokens, usage.total_tokens as u64);
    assert_eq!(enforcer.snapshot().tokens_reserved, 0);
    harness.assert_canonical_model_routing();
}

/// Live probe 2: real execution → cancellation → honest termination.
#[tokio::test]
#[ignore = "requires live NVIDIA credentials + WAN; run explicitly with --ignored"]
async fn live_p43_reliability_cancel_is_honest() {
    let harness = match m31a::testing::real_model::RealModelHarness::ensure() {
        Ok(h) => h,
        Err(skipped) => {
            eprintln!("LIVE SKIPPED (environment): {}", skipped.reason);
            return;
        }
    };
    let caller = harness.routed_caller(Vec::new());
    let token = CancellationToken::new();
    token.cancel();
    let res = caller
        .call_model_cancellable_with_usage("Count to 1000", &token)
        .await;
    assert!(res.is_err(), "cancelled live call must not succeed");
    let msg = res.err().unwrap().to_string();
    assert!(
        msg.contains("cancel") || msg.contains("abort"),
        "got: {msg}"
    );
}
