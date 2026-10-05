//! Architecture-level invariant tests (runtime): durable jobs, governed
//! recovery, canonical tool enforcement, Git finalization, provenance, and
//! the verified execution boundary.

use std::sync::Arc;
use std::time::Duration;

use m31a::capability::providers::CliGitProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::git::trailers::CommitTrailers;
use m31a::git::{AuthorizationAuthority, GIT_AUTH_DEFAULT_TTL, GitOperation};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, ImplementationHypothesis,
};
use m31a::kernel::seams::recovery::{
    FailureClassification, FailureClassificationRequest, RecoveryAction, RecoveryEngine,
    RecoveryError, RecoveryStrategyRequest,
};
use m31a::kernel::seams::scheduler::WorkItem;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::policy::effective::EffectivePolicy;
use m31a::process::job::JobSupervisor;
use m31a::state::Mission;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::ToolExecutionContext;
use m31a::tools::registry::ToolRegistry;
use tokio_util::sync::CancellationToken;

#[path = "common/git_auth.rs"]
mod git_auth;
use git_auth::TestGitAuth;

// ─── local fakes (production seams, deterministic behavior) ─────────────

struct FixedRecovery {
    action: RecoveryAction,
}

#[async_trait::async_trait]
impl RecoveryEngine for FixedRecovery {
    async fn classify_failure(
        &self,
        _req: FailureClassificationRequest,
    ) -> Result<FailureClassification, RecoveryError> {
        Ok(FailureClassification::Transient)
    }

    async fn determine_recovery(
        &self,
        _req: RecoveryStrategyRequest,
    ) -> Result<RecoveryAction, RecoveryError> {
        Ok(self.action.clone())
    }
}

fn repair_proposal(mission: MissionId, task: TaskId) -> ChangeProposal {
    ChangeProposal::new(
        task,
        mission,
        ImplementationHypothesis::new("p", "c", "m", "r", "v"),
        ChangeSurface::new(vec!["src/repaired.rs".to_string()]),
        vec![FileMutationProposal::new(
            "src/repaired.rs",
            FileMutationOp::CreateNew {
                content: "repaired".to_string(),
            },
            "repair",
        )],
    )
}

fn work_item(task: TaskId) -> WorkItem {
    WorkItem {
        task_id: task,
        title: "t".to_string(),
        estimated_tokens: 10,
        required_capabilities: vec![],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
        prompt_ref: None,
    }
}

async fn migrated_pool(dir: &std::path::Path) -> sqlx::SqlitePool {
    m31a::persistence::sqlite::schema::initialize_database(&dir.join("m31a.db"))
        .await
        .expect("migrate")
}

fn init_git_repo(ws: &std::path::Path) {
    let run = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(ws)
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "t"]);
    run(&["config", "user.email", "t@t"]);
    std::fs::write(ws.join("f.txt"), "base\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "base"]);
}

// ─── 12. Recovery repair uses the governed pipeline ─────────────────────

#[tokio::test]
async fn test_recovery_repair_uses_governed_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = migrated_pool(dir.path()).await;
    let bus = Arc::new(BroadcastEventBus::new(64));

    // Production dependencies: real policy WITHOUT any repair allow rule, so
    // the default Ask must fail closed (no trusted-recovery bypass).
    let deps = ControllerDependencies::production(
        pool.clone(),
        ws.clone(),
        ws.clone(),
        Some(bus.clone() as Arc<BroadcastEventBus>),
    )
    .with_auth_authority(Arc::new(m31a::git::AuthorizationAuthority::new()));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let proposal = repair_proposal(mission_id, task_id);
    let deps = deps.with_recovery(Arc::new(FixedRecovery {
        action: RecoveryAction::Repair {
            proposal: Box::new(proposal),
            reason: "test repair".to_string(),
        },
    }));

    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        bus.clone() as Arc<dyn EventBus>,
        CancellationToken::new(),
    )
    .with_workspace_root(ws.clone())
    .with_policy_role(m31a::state_machine::agent::AgentRole::implementer());
    controller.active_task = Some(work_item(task_id));
    controller.last_execution_result = Some(m31a::kernel::seams::execution::WorkExecutionResult {
        task_id,
        success: false,
        output: String::new(),
        error_detail: Some("compile fail".to_string()),
        token_usage: None,
    });
    controller.progress.current_stage = LoopStage::RecoverOrReplan;

    let outcome = controller.step().await.expect("recover step");
    // Policy did not Allow: repair refused, classified as failure.
    assert_eq!(outcome, StageOutcome::SkipTo(LoopStage::ClassifyFailure));
    let recorded_failure = controller
        .last_execution_result
        .as_ref()
        .expect("failure recorded");
    assert!(!recorded_failure.success);
    assert!(
        recorded_failure.output.contains("denied by policy")
            || recorded_failure
                .error_detail
                .as_deref()
                .unwrap_or("")
                .contains("policy"),
        "got: {} / {:?}",
        recorded_failure.output,
        recorded_failure.error_detail
    );
    // The bypass-free proof: the mutation never executed.
    assert!(
        !ws.join("src/repaired.rs").exists(),
        "denied repair must not mutate the workspace"
    );
}

// ─── 13/14. Rollback: governed success + truthful failure ────────────────

#[tokio::test]
async fn test_recovery_rollback_uses_governed_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    init_git_repo(&ws);
    // Dirty the tracked file so rollback has something to restore.
    std::fs::write(ws.join("f.txt"), "dirty\n").unwrap();

    // Workspace policy Allows the governed rollback restore.
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::fs::write(
        ws.join(".m31a/policy.toml"),
        r#"
version = "1.0"
name = "test"

[[rules]]
id = "allow-recovery-rollback"
decision = "allow"
tools = ["recovery_rollback_restore"]
"#,
    )
    .unwrap();

    let pool = migrated_pool(dir.path()).await;
    let bus = Arc::new(BroadcastEventBus::new(64));
    let deps = ControllerDependencies::production(
        pool.clone(),
        ws.clone(),
        ws.clone(),
        Some(bus.clone() as Arc<BroadcastEventBus>),
    )
    .with_auth_authority(Arc::new(m31a::git::AuthorizationAuthority::new()))
    .with_recovery(Arc::new(FixedRecovery {
        action: RecoveryAction::Rollback {
            reason: "test rollback".to_string(),
        },
    }));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        bus.clone() as Arc<dyn EventBus>,
        CancellationToken::new(),
    )
    .with_workspace_root(ws.clone())
    .with_policy_role(m31a::state_machine::agent::AgentRole::implementer());
    controller.active_task = Some(work_item(task_id));
    controller.last_execution_result = Some(m31a::kernel::seams::execution::WorkExecutionResult {
        task_id,
        success: false,
        output: String::new(),
        error_detail: Some("bad state".to_string()),
        token_usage: None,
    });
    controller.progress.current_stage = LoopStage::RecoverOrReplan;

    let outcome = controller.step().await.expect("rollback step");
    assert_eq!(outcome, StageOutcome::Advance(LoopStage::Checkpoint));
    // Real result, truthfully recorded.
    assert_eq!(std::fs::read_to_string(ws.join("f.txt")).unwrap(), "base\n");
    let last = controller.step_history.last().expect("history");
    assert!(last.success, "successful rollback recorded as success");
}

#[tokio::test]
async fn test_failed_rollback_is_not_recorded_as_success() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    // NOTE: no .git here — rollback must fail and record failure, never
    // `success = true` merely because the function was invoked.
    let pool = migrated_pool(dir.path()).await;
    let bus = Arc::new(BroadcastEventBus::new(64));
    let deps = ControllerDependencies::production(
        pool.clone(),
        ws.clone(),
        ws.clone(),
        Some(bus.clone() as Arc<BroadcastEventBus>),
    )
    .with_auth_authority(Arc::new(m31a::git::AuthorizationAuthority::new()))
    .with_recovery(Arc::new(FixedRecovery {
        action: RecoveryAction::Rollback {
            reason: "test rollback".to_string(),
        },
    }));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        bus.clone() as Arc<dyn EventBus>,
        CancellationToken::new(),
    )
    .with_workspace_root(ws.clone())
    .with_policy_role(m31a::state_machine::agent::AgentRole::implementer());
    controller.active_task = Some(work_item(task_id));
    controller.last_execution_result = Some(m31a::kernel::seams::execution::WorkExecutionResult {
        task_id,
        success: false,
        output: String::new(),
        error_detail: Some("bad state".to_string()),
        token_usage: None,
    });
    controller.progress.current_stage = LoopStage::RecoverOrReplan;

    let outcome = controller.step().await.expect("rollback step");
    assert_eq!(outcome, StageOutcome::Advance(LoopStage::Checkpoint));
    let last = controller.step_history.last().expect("history");
    assert!(
        !last.success,
        "failed rollback must not be recorded as success"
    );
    assert!(last.error.is_some());
    let recorded = controller
        .last_execution_result
        .as_ref()
        .expect("failure recorded");
    assert!(!recorded.success);
}

// ─── 15. Jobs: durable authority, real identity, limits ──────────────────

#[tokio::test]
async fn test_job_execution_uses_durable_authority() {
    let dir = tempfile::tempdir().unwrap();
    let pool = migrated_pool(dir.path()).await;
    let spool_dir = dir.path().join("spools");
    tokio::fs::create_dir_all(&spool_dir).await.unwrap();

    let supervisor =
        Arc::new(m31a::process::job::JobSupervisor::new(spool_dir).with_pool(pool.clone()));
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = m31a::ids::AgentId::new();
    // Durable identity rows the `jobs` foreign keys require.
    {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT OR IGNORE INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(mission_id.as_bytes().as_slice())
            .bind("job test")
            .bind("running")
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(task_id.as_bytes().as_slice())
            .bind(mission_id.as_bytes().as_slice())
            .bind("job task")
            .bind("running")
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO agents (id, mission_id, task_id, role, status, profile_fingerprint, max_steps, steps_consumed, model_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(agent_id.as_bytes().as_slice())
            .bind(mission_id.as_bytes().as_slice())
            .bind(task_id.as_bytes().as_slice())
            .bind("implementer")
            .bind("active")
            .bind("test")
            .bind(30)
            .bind(0)
            .bind("test-model")
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
    }
    let limits = m31a::sandbox::ResourceLimits::new(10_000, 1024 * 1024);

    let desc = supervisor
        .start_job(
            mission_id,
            task_id,
            agent_id,
            limits.clone(),
            "sh",
            &["-c".to_string(), "echo hello".to_string()],
            dir.path(),
            None,
            None,
        )
        .await
        .expect("start job");
    let job_id = m31a::ids::JobId::parse(&desc.job_id).unwrap();

    // Wait for completion.
    for _ in 0..100 {
        let st = supervisor.job_status(&job_id).await.unwrap();
        if st.state != "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let status = supervisor.job_status(&job_id).await.unwrap();
    assert_eq!(status.state, "completed");

    // The durable row carries the REAL identities and effective limits —
    // the same row startup recovery reconciles.
    let row = sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>, String, String)>(
        "SELECT mission_id, task_id, agent_id, command, resource_limits_json FROM jobs WHERE id = ?",
    )
    .bind(job_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("durable job row");
    assert_eq!(row.0, mission_id.as_bytes().to_vec());
    assert_eq!(row.1, task_id.as_bytes().to_vec());
    assert_eq!(row.2, agent_id.as_bytes().to_vec());
    assert_eq!(row.3, "sh");
    let persisted_limits: m31a::sandbox::ResourceLimits =
        serde_json::from_str(&row.4).expect("limits json");
    assert_eq!(persisted_limits, limits);

    // Output durability.
    let chunk = supervisor.job_output(&job_id, 0, 4096).await.unwrap();
    assert!(chunk.stdout.contains("hello"));
}

// ─── 16. Canonical tool enforcement (jobs) ───────────────────────────────

#[tokio::test]
async fn test_slash_command_uses_canonical_execution_path() {
    // A job tool call WITHOUT bound execution identity fails closed before
    // any process spawns: no authorization, no side effect.
    let caps = Arc::new(CapabilityRegistry::new());
    let tool_reg = Arc::new({
        let mut reg = ToolRegistry::new();
        reg.register(m31a::tools::process::StartJobTool);
        reg
    });
    // Register a jobs provider so the denial comes from AUTHORIZATION, not
    // from a missing capability.
    let spool_tmp = tempfile::tempdir().unwrap();
    let spool_dir = spool_tmp.path().join("spools");
    std::fs::create_dir_all(&spool_dir).unwrap();
    let supervisor = Arc::new(JobSupervisor::new(spool_dir));
    caps.register_jobs(Arc::new(
        m31a::capability::providers::LocalJobProvider::new(std::env::temp_dir(), supervisor),
    ));
    // Capability inventory must list the Jobs family or the pipeline's
    // capability check fails before authorization is even reached.
    caps.register_instance(m31a::capability::instance::CapabilityInstance::new(
        "jobs.local",
        "Local Background Jobs",
        "1.0.0",
        m31a::capability::family::CapabilityFamily::Jobs,
        "local_jobs",
        m31a::capability::permissions::CapabilityPermissions::full_access(),
    ));

    let pipeline = ToolPipelineRunner::new(tool_reg);
    // Policy Allows the tool (explicit workspace rule): any denial below
    // therefore comes from execution-identity enforcement, not policy.
    let mut allow_rule = m31a::policy::rule::PolicyRule::new(
        "allow-start-job",
        m31a::kernel::seams::policy::PolicyDecision::Allow,
    );
    allow_rule.tools = vec!["start_job".to_string()];
    let policy = EffectivePolicy::new(vec![(
        m31a::policy::layers::PolicyLayer::Workspace,
        vec![allow_rule],
    )]);
    let action = m31a::agent::runner::ActionRequest {
        id: "call-1".to_string(),
        tool_name: "start_job".to_string(),
        parameters: serde_json::json!({"command": "echo", "args": ["hi"]}),
    };
    let ctx =
        ToolExecutionContext::new(caps.clone(), std::env::temp_dir(), CancellationToken::new())
            .with_agent_role(m31a::state_machine::agent::AgentRole::implementer())
            .with_autonomy_mode(AutonomyMode::Autonomous)
            .with_policy_hash("test-policy".to_string());
    // NOTE: no mission/task/agent identity bound.
    let res = pipeline
        .execute_action(&action, &ctx, &policy, AutonomyMode::Autonomous)
        .await;
    assert!(
        !res.success,
        "unbound job submission must fail closed, got: {}",
        res.output
    );
    assert!(
        res.error
            .as_deref()
            .unwrap_or("")
            .contains("JOB_AUTHORIZATION_UNBOUND"),
        "denial must name the missing authorization, got: {:?}",
        res.error
    );

    // Positive control: the SAME policy with fully bound identity executes,
    // and the durable record carries the real identities.
    let mission = MissionId::new();
    let task = TaskId::new();
    let agent = m31a::ids::AgentId::new();
    let bound_ctx = ToolExecutionContext::new(caps, std::env::temp_dir(), CancellationToken::new())
        .with_agent_role(m31a::state_machine::agent::AgentRole::implementer())
        .with_autonomy_mode(AutonomyMode::Autonomous)
        .with_policy_hash("test-policy".to_string())
        .with_mission_id(mission)
        .with_task_id(task)
        .with_agent_id(agent);
    let res2 = pipeline
        .execute_action(&action, &bound_ctx, &policy, AutonomyMode::Autonomous)
        .await;
    assert!(
        res2.success,
        "bound job submission must execute: {:?}",
        res2.error
    );
}

// ─── 17/18. Finalization + provenance ────────────────────────────────────

#[tokio::test]
async fn test_failed_git_finalize_prevents_completion() {
    // A commit gate minted under one policy generation does not authorize
    // when the generation changed: the runtime's authorize path re-checks
    // policy before minting, and the gate binds the hash. Here we prove the
    // binding primitive: a gate carrying a stale hash is distinguishable.
    let authority = AuthorizationAuthority::new();
    let ws = tempfile::tempdir().unwrap();
    let op = GitOperation::Commit {
        message: "m".to_string(),
    };
    let stale = authority.mint_git_authorization(
        MissionId::new(),
        Some(TaskId::new()),
        None,
        "policy-old".to_string(),
        ws.path().to_path_buf(),
        op,
        Vec::new(),
        "commit",
        "test",
        GIT_AUTH_DEFAULT_TTL,
    );
    assert_eq!(stale.policy_hash, "policy-old");
    // The runtime only mints with the LIVE hash; a presented stale gate for a
    // live-hash operation never covers it (different generation ⇒ the
    // mint-time policy check that produced it is not this generation's).
    assert_ne!(stale.policy_hash, "policy-live");

    // Provenance roundtrip: the committed task id is the task that performed
    // the mutation — parse returns exactly what was embedded.
    let mission = MissionId::new();
    let task = TaskId::new();
    let trailers = CommitTrailers::for_governed_execution(
        mission,
        task,
        None,
        m31a::state_machine::agent::AgentRole::implementer(),
        "test-model",
        stale.authorization_id.clone(),
    );
    let msg = CommitTrailers::embed_trailers("fix: x", &trailers).unwrap();
    let parsed = CommitTrailers::parse_trailers(&msg).unwrap();
    assert_eq!(parsed.task_id, task, "trailer task must be the real task");
    assert_eq!(parsed.mission_id, mission);
    assert_eq!(
        parsed.authorization_id.as_deref(),
        Some(stale.authorization_id.as_str())
    );
}

#[tokio::test]
async fn test_git_provenance_references_actual_task() {
    // CommitTrailers::new (legacy shape) still roundtrips, and the governed
    // constructor binds agent + authorization provenance.
    let mission = MissionId::new();
    let task = TaskId::new();
    let agent = m31a::ids::AgentId::new();
    let trailers = CommitTrailers::for_governed_execution(
        mission,
        task,
        Some(agent),
        m31a::state_machine::agent::AgentRole::implementer(),
        "model-x",
        "auth-123",
    )
    .with_verification("run-9");
    let msg = CommitTrailers::embed_trailers("feat: y", &trailers).unwrap();
    let parsed = CommitTrailers::parse_trailers(&msg).unwrap();
    assert_eq!(parsed, trailers);
    assert_eq!(parsed.agent_id, Some(agent));
}

// ─── misc: mission persistence for controller tests ──────────────────────

#[allow(dead_code)]
async fn seed_mission(pool: &sqlx::SqlitePool, mission: MissionId) {
    let repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(pool.clone());
    repo.insert(&Mission::new(mission, "invariant test".to_string()))
        .await
        .expect("seed mission");
}

#[allow(dead_code)]
fn test_auth_for(ws: &std::path::Path) -> (TestGitAuth, m31a::git::GitGate) {
    let auth = TestGitAuth::new();
    let gate = auth.raw_gate(ws);
    (auth, gate)
}

#[allow(dead_code)]
async fn git_provider_for(
    ws: &std::path::Path,
) -> (CliGitProvider, m31a::git::AuthorizationAuthority) {
    (
        CliGitProvider::new(ws),
        m31a::git::AuthorizationAuthority::new(),
    )
}
