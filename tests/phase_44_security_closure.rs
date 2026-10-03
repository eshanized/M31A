//! Phase 44 — Security Boundary & Durable Recovery Closure.
//!
//! Adversarial proof that no hostile input, crash, restart, corrupted record,
//! or interrupted operation can make M31A violate its governance, isolation,
//! accounting, or evidence laws:
//!
//! ```text
//! hostile input / crash / restart / corruption / interruption
//!   → validated / scrubbed / fenced / revalidated / transacted
//!   → typed failure or honest continuation (never bypass, never fake)
//! ```
//!
//! Live probes are partitioned with `#[ignore]` per
//! `tests/architecture_live_test_partition.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::budget::{BudgetEnforcer, TaskEstimates};
use m31a::capability::providers::LocalFileSystemProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::fs::FileSystemService;
use m31a::change::authority::ChangeAuthority;
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::events::bus::BroadcastEventBus;
use m31a::git::{GitGate, MergeStrategy, validate_git_ref_arg, validate_git_sha};
use m31a::ids::{AgentId, ArtifactId, CheckpointId, MissionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, ImplementationHypothesis,
};
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::kernel::seams::recovery::FailureClassification;
use m31a::model::types::{TokenUsage, UsageSource};
use m31a::persistence::artifacts::fs_store::FsArtifactStore;
use m31a::persistence::sqlite::repositories::SqliteLifecycleRepository;
use m31a::persistence::sqlite::repositories::lifecycle::{ResumeAuthError, ResumeAuthVerdict};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::dedup::{
    FenceVerdict, MutationDedupFence, canonical_json, fingerprint_mutation, is_fenced_mutating_tool,
};
use m31a::planning::review::{PreExecutionCoordinator, PreExecutionResponse, TaskRevision};
use m31a::process::hardened::HardenedSpawn;
use m31a::state::budget::ResourceBudget;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::ToolExecutionContext;
use m31a::tools::registry::ToolRegistry;

// ===========================================================================
// HELPERS
// ===========================================================================

async fn setup_test_db() -> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("phase44.db");
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
        .bind("phase 44 mission")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("phase 44 task")
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

/// Drive the governed chain to ReadyToExecute (plan rev 1, task rev 1, valid auth).
async fn drive_to_authorized(
    pool: &sqlx::SqlitePool,
    bus: &Arc<BroadcastEventBus>,
    workspace: &std::path::Path,
    session_id: &str,
) -> m31a::planning::review::ExecutionAuthorization {
    let coordinator = coordinator_for(pool, bus, workspace);
    let resp = coordinator
        .init_intent(session_id, "Build a microservice API in Rust", "operator")
        .await
        .expect("init");
    assert!(matches!(resp, PreExecutionResponse::PlanForReview { .. }));
    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.to_string()),
            },
            "operator",
        )
        .await
        .unwrap();
    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.to_string()),
            },
            "operator",
        )
        .await
        .unwrap();
    match coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.to_string()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .unwrap()
    {
        PreExecutionResponse::ReadyToExecute { authorization, .. } => authorization,
        other => panic!("expected ReadyToExecute, got {other:?}"),
    }
}

struct AllowGate;
#[async_trait]
impl PolicyGate for AllowGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Allow)
    }
}

fn git_repo_with_divergence() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let repo = dir.path().to_path_buf();
    let run = |args: &[&str]| {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "t"]);
    run(&["config", "user.email", "t@t"]);
    std::fs::write(repo.join("f.txt"), b"base\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "base"]);
    run(&["checkout", "-b", "branch-a"]);
    std::fs::write(repo.join("f.txt"), b"a-side\n").unwrap();
    run(&["commit", "-am", "a"]);
    run(&["checkout", "main"]);
    run(&["checkout", "-b", "branch-b"]);
    std::fs::write(repo.join("f.txt"), b"b-side\n").unwrap();
    run(&["commit", "-am", "b"]);
    (dir, repo)
}

fn tip(repo: &Path, branch: &str) -> String {
    String::from_utf8_lossy(
        &Command::new("git")
            .args(["rev-parse", branch])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string()
}

// ===========================================================================
// PROCESS ADVERSARIAL (§22)
// ===========================================================================

#[tokio::test]
async fn p44_process_secret_env_never_inherited() {
    let dir = tempdir().unwrap();
    // Forbidden keys are rejected at configuration time (deterministic).
    for key in [
        "API_KEY_NVIDIA",
        "MY_SECRET_TOKEN",
        "AWS_PASSWORD",
        "LD_PRELOAD",
        "GIT_DIR",
        "NVIDIA_MODEL_KEY",
    ] {
        assert!(
            HardenedSpawn::new(dir.path())
                .with_env_var(key, "evil")
                .is_err(),
            "forbidden env key '{key}' must be rejected"
        );
    }
    // Allowed keys pass through; the child observes exactly them.
    let spawn = HardenedSpawn::new(dir.path())
        .with_env_var("M44_PROBE", "present-123")
        .unwrap();
    let mut cmd = spawn
        .build_command(
            "sh",
            &["-c".to_string(), "echo $M44_PROBE".to_string()],
            None,
        )
        .expect("hardened build");
    let out = cmd.output().await.expect("run probe");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "present-123");

    // The scrubbed environment carries the trusted baseline but no secrets:
    // even when the host exports credentials, the child map excludes them.
    unsafe { std::env::set_var("M44_HOST_SECRET_TOKEN_XYZ", "host-leak") };
    let builder = m31a::process::env::EnvironmentBuilder::new(dir.path());
    let map = builder.build_map();
    assert!(
        !map.keys().any(|k| k.contains("M44_HOST_SECRET_TOKEN_XYZ")),
        "host secret must be scrubbed, got keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
    assert!(map.contains_key("PATH"), "trusted baseline PATH present");
    unsafe { std::env::remove_var("M44_HOST_SECRET_TOKEN_XYZ") };
}

#[tokio::test]
async fn p44_process_malicious_workdir_rejected_before_spawn() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let spawn = HardenedSpawn::new(&ws);
    // Outside root.
    assert!(
        spawn
            .build_command("echo", &["hi".to_string()], Some(Path::new("/tmp")))
            .is_err()
    );
    // Protected component as cwd.
    let git_dir = ws.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    assert!(
        spawn
            .build_command("echo", &["hi".to_string()], Some(&git_dir))
            .is_err()
    );
    // Empty program.
    assert!(spawn.build_command("", &[], Some(&ws)).is_err());
    // Dangerous redirection args.
    assert!(
        spawn
            .build_command(
                "git",
                &["--git-dir=/tmp/x".to_string(), "status".to_string()],
                Some(&ws)
            )
            .is_err()
    );
}

#[tokio::test]
async fn p44_job_manager_hostile_submit_fails_closed() {
    let (tmp, pool, _bus) = setup_test_db().await;
    let (_mission_id, task_id, agent_id) = seed_mission_task_agent(&pool).await;
    let mission_id = MissionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("job mission").bind("active").bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    let store = Arc::new(FsArtifactStore::new(tmp.path().join("artifacts")));
    let mgr = m31a::process::job::JobManager::new(
        pool.clone(),
        Arc::new(m31a::process::admission::JobAdmissionController::new(4, 2)),
        store,
        tmp.path().join("spools"),
    )
    .with_workspace_root(tmp.path().to_path_buf());

    // Malicious working directory outside the pinned root.
    let bad_cwd = m31a::process::job::SubmitJobRequest {
        mission_id,
        task_id,
        agent_id,
        tool_call_id: None,
        command: "echo".to_string(),
        args: vec!["hi".to_string()],
        working_dir: PathBuf::from("/tmp"),
        provider: "test".to_string(),
        resource_limits: m31a::sandbox::ResourceLimits::default(),
        timeout: Some(Duration::from_secs(10)),
    };
    let job_id = mgr.submit_job(bad_cwd).await.expect("submit accepted");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let rec = mgr.job_status(&job_id).await.expect("status readable");
    assert_eq!(rec.state, m31a::process::job::JobState::Failed);
    assert!(
        rec.failure_reason
            .unwrap_or_default()
            .contains("security boundary"),
        "rejection must name the boundary"
    );

    // Dangerous git-redirection args.
    let bad_args = m31a::process::job::SubmitJobRequest {
        mission_id,
        task_id,
        agent_id,
        tool_call_id: None,
        command: "git".to_string(),
        args: vec!["--git-dir=/tmp/x".to_string(), "status".to_string()],
        working_dir: tmp.path().to_path_buf(),
        provider: "test".to_string(),
        resource_limits: m31a::sandbox::ResourceLimits::default(),
        timeout: Some(Duration::from_secs(10)),
    };
    let job_id2 = mgr.submit_job(bad_args).await.expect("submit accepted");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let rec2 = mgr.job_status(&job_id2).await.expect("status readable");
    assert_eq!(rec2.state, m31a::process::job::JobState::Failed);
}

#[tokio::test]
async fn p44_job_manager_timeout_and_cancel() {
    let (tmp, pool, _bus) = setup_test_db().await;
    let (_m, task_id, agent_id) = seed_mission_task_agent(&pool).await;
    let mission_id = MissionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("job mission").bind("active").bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    let store = Arc::new(FsArtifactStore::new(tmp.path().join("artifacts")));
    let mgr = m31a::process::job::JobManager::new(
        pool.clone(),
        Arc::new(m31a::process::admission::JobAdmissionController::new(4, 2)),
        store,
        tmp.path().join("spools"),
    )
    .with_workspace_root(tmp.path().to_path_buf());
    let base = |cmd: &str, args: Vec<String>, timeout: Option<Duration>| {
        m31a::process::job::SubmitJobRequest {
            mission_id,
            task_id,
            agent_id,
            tool_call_id: None,
            command: cmd.to_string(),
            args,
            working_dir: tmp.path().to_path_buf(),
            provider: "test".to_string(),
            resource_limits: m31a::sandbox::ResourceLimits::default(),
            timeout,
        }
    };

    // Timeout kills the child and reports honestly.
    let t1 = mgr
        .submit_job(base(
            "sleep",
            vec!["30".to_string()],
            Some(Duration::from_secs(1)),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(
        mgr.job_status(&t1).await.unwrap().state,
        m31a::process::job::JobState::TimedOut
    );

    // Cancellation terminates and reports Cancelled (no unsafe half-operation).
    let t2 = mgr
        .submit_job(base(
            "sleep",
            vec!["30".to_string()],
            Some(Duration::from_secs(60)),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    mgr.cancel_job(&t2, Duration::from_secs(5)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        mgr.job_status(&t2).await.unwrap().state,
        m31a::process::job::JobState::Cancelled
    );
}

#[tokio::test]
async fn p44_job_manager_output_bound_truncates_honestly() {
    use m31a::process::spool::{
        DEFAULT_MAX_RING_BYTES, DEFAULT_MAX_RING_LINES, DualBufferOutput, StreamType,
    };
    let dir = tempdir().unwrap();
    let job_id = m31a::ids::JobId::new();
    let spool = DualBufferOutput::new(
        job_id,
        dir.path().to_path_buf(),
        DEFAULT_MAX_RING_LINES,
        DEFAULT_MAX_RING_BYTES,
    )
    .unwrap()
    .with_max_spool_bytes(100);
    spool.append(StreamType::Stdout, &[b'x'; 500]).unwrap();
    assert!(spool.truncated(), "cap must engage");
    let len = std::fs::metadata(dir.path().join(format!("job_{job_id}_stdout.spool")))
        .unwrap()
        .len();
    assert!(len <= 100, "durable spool bounded, got {len}");
}

#[tokio::test]
async fn p44_verification_provider_contained() {
    use m31a::capability::VerificationService;
    use m31a::capability::providers::LocalVerificationProvider;
    use m31a::capability::traits::verification::{VerificationKind, VerificationTarget};
    let dir = tempdir().unwrap();
    let prov = LocalVerificationProvider::new(dir.path());

    // Custom program escaping the workspace is denied.
    let evil = VerificationTarget {
        name: "evil".to_string(),
        kind: VerificationKind::Custom("/bin/sh".to_string()),
        args: vec!["-c".to_string(), "echo pwned".to_string()],
    };
    let err = prov
        .run_verification(&evil)
        .await
        .expect_err("escape denied");
    assert!(!err.to_string().is_empty());

    // Dangerous args are denied even for fixed programs.
    let evil_args = VerificationTarget {
        name: "evil-args".to_string(),
        kind: VerificationKind::Custom("cargo".to_string()),
        args: vec!["--git-dir=/tmp/x".to_string()],
    };
    // "cargo" without '/' skips the path check but safety validation denies.
    let res = prov.run_verification(&evil_args).await;
    assert!(res.is_err(), "dangerous args must be denied, got {res:?}");

    // A benign custom program inside the workspace runs scrubbed + bounded.
    let script = dir.path().join("check.sh");
    std::fs::write(&script, "#!/bin/sh\necho verified-ok\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let ok = VerificationTarget {
        name: "ok".to_string(),
        kind: VerificationKind::Custom("./check.sh".to_string()),
        args: vec![],
    };
    let report = prov.run_verification(&ok).await.expect("contained run");
    assert!(report.passed);
    assert!(report.output.contains("verified-ok"));
}

// ===========================================================================
// GIT ADVERSARIAL (§22) + SEMANTICS (§8)
// ===========================================================================

#[test]
fn p44_git_ref_validation_table() {
    for evil in [
        "", "-evil", "--help", "a..b", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a@{b",
        "a/", "a.lock", "a b", "a\nb",
    ] {
        assert!(
            validate_git_ref_arg("branch", evil).is_err(),
            "'{evil}' must be rejected"
        );
    }
    for good in ["main", "m31a/abc-123", "feature/x-y_z", "v1.2.3"] {
        assert!(
            validate_git_ref_arg("branch", good).is_ok(),
            "'{good}' must be accepted"
        );
    }
    assert!(validate_git_sha("sha", "0123456789abcdef0123456789abcdef01234567").is_ok());
    assert!(validate_git_sha("sha", "deadbeef").is_err());
    assert!(validate_git_sha("sha", "").is_err());
}

#[tokio::test]
async fn p44_git_destructive_without_approval_fails_closed() {
    let (_dir, repo) = git_repo_with_divergence();
    let denied = GitGate::denied();
    let wt = m31a::git::worktree::IsolatedWorktree {
        mission_id: MissionId::new(),
        path: repo.join("wt"),
        branch: "-evil".to_string(),
        base_commit: "HEAD".to_string(),
        created_at: chrono::Utc::now(),
    };
    // Injection closed before policy is even reached.
    let mgr =
        m31a::git::worktree::WorktreeManager::new(m31a::git::worktree::WorktreeConfig::new(&repo));
    assert!(mgr.remove_worktree(&wt, true, &denied).await.is_err());
    assert!(
        mgr.remove_worktree(&wt, true, &GitGate::authorized())
            .await
            .is_err(),
        "leading-dash branch rejected regardless of approval"
    );

    // Policy denial precedes any mutation.
    let wt_ok = m31a::git::worktree::IsolatedWorktree {
        branch: "branch-a".to_string(),
        ..wt
    };
    let mut machine = m31a::git::integration::WorktreeIntegrationStateMachine::new(&repo);
    let err = machine
        .integrate(&wt_ok, "branch-b", MergeStrategy::MergeCommit, &denied)
        .await
        .expect_err("denied gate must fail closed");
    assert!(err.to_string().contains("approval"), "got: {err}");

    // Stash paths likewise (on a dirty TRACKED tree, so gated mutations
    // are reached; untracked-only dirt yields an honest Ok(None)).
    std::fs::write(repo.join("f.txt"), b"uncommitted modification\n").unwrap();
    let stash = m31a::git::PrivateStashManager::new(&repo);
    assert!(stash.save(&MissionId::new(), None, &denied).await.is_err());
    assert!(stash.apply(&MissionId::new(), &denied).await.is_err());

    // Malicious target refs rejected even when authorized.
    let err = machine
        .integrate(
            &wt_ok,
            "-b",
            MergeStrategy::MergeCommit,
            &GitGate::authorized(),
        )
        .await
        .expect_err("malicious ref rejected");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn p44_git_strategy_semantics_exact() {
    // FastForwardOnly on divergent branches: no merge commit, target unmoved.
    let (_dir, repo) = git_repo_with_divergence();
    let before = tip(&repo, "branch-b");
    let wt = m31a::git::worktree::IsolatedWorktree {
        mission_id: MissionId::new(),
        path: repo.join("wt-ff"),
        branch: "branch-a".to_string(),
        base_commit: "HEAD".to_string(),
        created_at: chrono::Utc::now(),
    };
    let mut machine = m31a::git::integration::WorktreeIntegrationStateMachine::new(&repo);
    let rep = machine
        .integrate(
            &wt,
            "branch-b",
            MergeStrategy::FastForwardOnly,
            &GitGate::authorized(),
        )
        .await
        .expect("ff-only reports");
    assert_eq!(rep.state, m31a::git::IntegrationState::IntegrationConflict);
    assert!(
        rep.merge_commit.is_none(),
        "ff-only must never fabricate a merge commit"
    );
    assert_eq!(tip(&repo, "branch-b"), before, "target ref untouched");

    // PreferFastForward on divergent-but-mergable branches: ff-only fails,
    // fallback produces a real merge commit and advances the target.
    let (dir2, repo2) = git_repo_with_clean_divergence();
    let _ = dir2;
    let before2 = tip(&repo2, "branch-b");
    let wt2 = m31a::git::worktree::IsolatedWorktree {
        mission_id: MissionId::new(),
        path: repo2.join("wt-pff"),
        branch: "branch-a".to_string(),
        base_commit: "HEAD".to_string(),
        created_at: chrono::Utc::now(),
    };
    let mut machine2 = m31a::git::integration::WorktreeIntegrationStateMachine::new(&repo2);
    let rep2 = machine2
        .integrate(
            &wt2,
            "branch-b",
            MergeStrategy::PreferFastForward,
            &GitGate::authorized(),
        )
        .await
        .expect("prefer-ff integrates");
    assert_eq!(rep2.state, m31a::git::IntegrationState::Integrated);
    assert!(rep2.merge_commit.is_some());
    assert_ne!(tip(&repo2, "branch-b"), before2, "target advanced by merge");
}

/// Branches diverged on disjoint files: fast-forward impossible, merge clean.
fn git_repo_with_clean_divergence() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let repo = dir.path().to_path_buf();
    let run = |args: &[&str]| {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success(),
            "git {args:?}"
        );
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "t"]);
    run(&["config", "user.email", "t@t"]);
    std::fs::write(repo.join("f.txt"), b"base\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "base"]);
    run(&["checkout", "-b", "branch-a"]);
    std::fs::write(repo.join("a.txt"), b"a-side\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "a"]);
    run(&["checkout", "main"]);
    run(&["checkout", "-b", "branch-b"]);
    std::fs::write(repo.join("b.txt"), b"b-side\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-m", "b"]);
    (dir, repo)
}

#[tokio::test]
async fn p44_git_merge_conflict_leaves_target_untouched() {
    let (_dir, repo) = git_repo_with_divergence();
    let before = tip(&repo, "branch-b");
    let wt = m31a::git::worktree::IsolatedWorktree {
        mission_id: MissionId::new(),
        path: repo.join("wt-c"),
        branch: "branch-a".to_string(),
        base_commit: "HEAD".to_string(),
        created_at: chrono::Utc::now(),
    };
    let mut machine = m31a::git::integration::WorktreeIntegrationStateMachine::new(&repo);
    let rep = machine
        .integrate(
            &wt,
            "branch-b",
            MergeStrategy::MergeCommit,
            &GitGate::authorized(),
        )
        .await
        .expect("conflict report");
    assert_eq!(rep.state, m31a::git::IntegrationState::IntegrationConflict);
    assert!(rep.merge_commit.is_none());
    assert!(!rep.conflict_files.is_empty());
    assert_eq!(tip(&repo, "branch-b"), before);
}

// ===========================================================================
// FILESYSTEM ADVERSARIAL (§22) + READER CONTAINMENT (§10)
// ===========================================================================

#[test]
fn p44_scanner_symlink_farm_terminates_contained() {
    use m31a::repo::scanner::RepositoryScanner;
    let dir = tempdir().unwrap();
    let ws = dir.path();
    std::fs::write(ws.join("real.rs"), b"pub fn f() {}\n").unwrap();
    std::fs::create_dir_all(ws.join("sub")).unwrap();
    std::fs::write(ws.join("sub").join("inner.rs"), b"pub fn g() {}\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/", ws.join("link_root"));
        let _ = symlink("..", ws.join("sub").join("link_up"));
        let _ = symlink("cycle_b", ws.join("cycle_a"));
        let _ = symlink("cycle_a", ws.join("cycle_b"));
        let _ = symlink("real.rs", ws.join("link_file.rs"));
        let _ = symlink("sub", ws.join("link_sub"));
    }
    let scanner = RepositoryScanner::new(ws);
    let summary = scanner.scan().expect("scan terminates");
    // Real files indexed; symlink targets never escape the workspace.
    assert!(summary.files.iter().any(|f| f.relative_path == "real.rs"));
    assert!(
        summary
            .files
            .iter()
            .any(|f| f.relative_path.ends_with("inner.rs"))
    );
    assert!(
        !summary
            .files
            .iter()
            .any(|f| f.relative_path.contains("link_")),
        "symlinks must not be indexed: {:?}",
        summary
            .files
            .iter()
            .map(|f| &f.relative_path)
            .collect::<Vec<_>>()
    );
    for f in &summary.files {
        assert!(
            !Path::new(&f.relative_path).is_absolute(),
            "no absolute escape: {}",
            f.relative_path
        );
    }
}

#[tokio::test]
async fn p44_reconciler_traversal_and_protected_fail_closed() {
    use m31a::change::reconciler::PreMutationReconciler;
    let dir = tempdir().unwrap();
    let ws = dir.path();
    std::fs::write(ws.join("ok.rs"), b"pub fn ok() {}\n").unwrap();
    let task_id = TaskId::new();
    let mission_id = MissionId::new();
    for evil in [
        "../escape.rs",
        "/etc/passwd",
        ".git/config",
        ".m31a/x",
        "a/../../b.rs",
    ] {
        let hypothesis = ImplementationHypothesis::new("h", "c", "x", "y", "t");
        let surface = ChangeSurface::new(vec![evil.to_string()]);
        let proposal = ChangeProposal::new(
            task_id,
            mission_id,
            hypothesis,
            surface,
            vec![FileMutationProposal::new(
                evil,
                FileMutationOp::Substring {
                    old_content: "a".to_string(),
                    new_content: "b".to_string(),
                },
                "evil",
            )],
        );
        let report = PreMutationReconciler::reconcile(
            ws,
            &proposal,
            None,
            None,
            &Default::default(),
            Some(task_id),
        );
        assert!(!report.is_valid, "'{evil}' must be invalid");
    }
}

#[tokio::test]
async fn p44_observer_escape_fails_closed() {
    use m31a::change::observer::PostMutationObserver;
    let dir = tempdir().unwrap();
    let err = PostMutationObserver::observe(dir.path(), &["../outside.rs".to_string()], None)
        .await
        .expect_err("escape must fail closed");
    assert!(!err.to_string().is_empty());
}

#[test]
fn p44_canonical_json_stable() {
    let a = serde_json::json!({"z": 1, "a": {"d": 4, "c": 3}, "m": [3, 2, 1]});
    let b = serde_json::json!({"m": [3, 2, 1], "a": {"c": 3, "d": 4}, "z": 1});
    assert_eq!(canonical_json(&a), canonical_json(&b));
    assert!(is_fenced_mutating_tool("write_file"));
    assert!(is_fenced_mutating_tool("edit_file"));
    assert!(!is_fenced_mutating_tool("run_command"));
    assert!(!is_fenced_mutating_tool("run_tests"));
}

// ===========================================================================
// RECOVERY ADVERSARIAL (§22): STALE AUTH / DUP / PARTIAL / RESTART
// ===========================================================================

#[tokio::test]
async fn p44_resume_valid_binding_permits_resume() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;
    let auth = drive_to_authorized(&pool, &bus, dir.path(), &session_id).await;
    // Drive lifecycle into Executing via the validated seam.
    let repo = SqliteLifecycleRepository::new(pool.clone());
    repo.save_validated_transition(
        &session_id,
        m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
        m31a::state_machine::lifecycle::LifecycleEvent::StartExecution,
    )
    .await
    .expect("enter Executing");
    match repo
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect("revalidation runs")
    {
        ResumeAuthVerdict::Valid {
            plan_revision,
            task_revision,
            ..
        } => {
            assert_eq!(plan_revision, auth.plan_revision);
            assert_eq!(task_revision, auth.task_revision);
        }
        ResumeAuthVerdict::NotRequired { reason } => {
            panic!("expected Valid, got NotRequired({reason})")
        }
    }
}

#[tokio::test]
async fn p44_resume_task_change_after_auth_halts() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;
    drive_to_authorized(&pool, &bus, dir.path(), &session_id).await;
    let repo = SqliteLifecycleRepository::new(pool.clone());
    repo.save_validated_transition(
        &session_id,
        m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
        m31a::state_machine::lifecycle::LifecycleEvent::StartExecution,
    )
    .await
    .unwrap();
    // Attack: new task revision appears after authorization.
    let rev1 = repo
        .load_latest_task_revision(&session_id)
        .await
        .unwrap()
        .unwrap();
    let rev2 = TaskRevision::new(
        &session_id,
        rev1.revision + 1,
        rev1.plan_revision,
        rev1.tasks.clone(),
        "attacker",
        m31a::planning::review::RevisionAuthorType::User,
        Some(rev1.revision),
    );
    repo.save_task_revision(&rev2).await.unwrap();
    let err = repo
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect_err("task change must halt resume");
    assert!(matches!(err, ResumeAuthError::Stale(_)), "got: {err}");
}

#[tokio::test]
async fn p44_resume_hash_tamper_after_auth_halts() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;
    drive_to_authorized(&pool, &bus, dir.path(), &session_id).await;
    let repo = SqliteLifecycleRepository::new(pool.clone());
    repo.save_validated_transition(
        &session_id,
        m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
        m31a::state_machine::lifecycle::LifecycleEvent::StartExecution,
    )
    .await
    .unwrap();
    // Attack: authorization row rebound to foreign hashes behind the runtime.
    sqlx::query("UPDATE execution_authorizations SET plan_content_hash = 'deadbeef', task_content_hash = 'deadbeef'")
        .execute(&pool)
        .await
        .unwrap();
    let err = repo
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect_err("hash tamper must halt resume");
    assert!(matches!(err, ResumeAuthError::Stale(_)), "got: {err}");
}

#[tokio::test]
async fn p44_resume_no_session_is_not_required() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let repo = SqliteLifecycleRepository::new(pool.clone());
    match repo
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect("runs")
    {
        ResumeAuthVerdict::NotRequired { .. } => {}
        v => panic!("expected NotRequired, got {v:?}"),
    }
}

#[tokio::test]
async fn p44_resume_malformed_auth_row_fails_closed() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;
    drive_to_authorized(&pool, &bus, dir.path(), &session_id).await;
    let repo = SqliteLifecycleRepository::new(pool.clone());
    repo.save_validated_transition(
        &session_id,
        m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
        m31a::state_machine::lifecycle::LifecycleEvent::StartExecution,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE execution_authorizations SET decision = 'bogus_decision'")
        .execute(&pool)
        .await
        .unwrap();
    let err = repo
        .revalidate_authorization_for_resume(mission_id)
        .await
        .expect_err("malformed auth must fail closed");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
}

#[tokio::test]
async fn p44_fence_suppresses_duplicate_file_mutation() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let ws = dir.path();
    let caps = Arc::new(CapabilityRegistry::new());
    caps.register_instance(m31a::capability::instance::CapabilityInstance::new(
        "fs.phase44",
        "Phase 44 Filesystem",
        "1.0.0",
        m31a::capability::family::CapabilityFamily::Filesystem,
        "phase44_fs",
        m31a::capability::permissions::CapabilityPermissions::full_access(),
    ));
    caps.register_filesystem(Arc::new(LocalFileSystemProvider::new(ws).unwrap()));
    let registry = Arc::new(ToolRegistry::new_default(Arc::clone(&caps)));
    let runner =
        m31a::pipeline::runner::ToolPipelineRunner::new(registry).with_db_pool(pool.clone());
    let ctx = || {
        ToolExecutionContext::new(caps.clone(), ws.to_path_buf(), CancellationToken::new())
            .with_mission_id(mission_id)
            .with_task_id(task_id)
    };
    let action = m31a::agent::runner::ActionRequest {
        id: "act-1".to_string(),
        tool_name: "write_file".to_string(),
        parameters: serde_json::json!({"path": "note.txt", "content": "hello fence"}),
    };
    let r1 = runner
        .execute_action(&action, &ctx(), &AllowGate, AutonomyMode::Safe)
        .await;
    assert!(r1.success, "first write executes: {:?}", r1.error);
    let r2 = runner
        .execute_action(&action, &ctx(), &AllowGate, AutonomyMode::Safe)
        .await;
    assert!(r2.success, "duplicate suppressed as success");
    assert!(
        r2.output.contains("duplicate suppressed"),
        "got: {}",
        r2.output
    );
    assert_eq!(
        tokio::fs::read_to_string(ws.join("note.txt"))
            .await
            .unwrap(),
        "hello fence"
    );
    // Exactly one committed success recorded.
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tool_mutation_fence WHERE outcome = 'success'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn p44_fence_failure_permits_legitimate_retry() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (_, task_id, _) = seed_mission_task_agent(&pool).await;
    let fence = MutationDedupFence::new(pool.clone());
    let params = serde_json::json!({"path": "x.txt", "content": "v"});
    let fp = fingerprint_mutation(&task_id, "write_file", &params);
    // A recorded FAILURE must not suppress re-execution.
    fence
        .record(&task_id, "write_file", &fp, false, None, None)
        .await;
    assert!(matches!(
        fence.check(&task_id, "write_file", &fp).await,
        FenceVerdict::Proceed
    ));
    // Success then suppresses.
    fence
        .record(&task_id, "write_file", &fp, true, Some("abc"), None)
        .await;
    assert!(matches!(
        fence.check(&task_id, "write_file", &fp).await,
        FenceVerdict::DuplicateSuppressed { .. }
    ));
}

#[tokio::test]
async fn p44_fence_lane_b_duplicate_replays_without_side_effect() {
    let (dir, pool, _bus) = setup_test_db().await;
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());
    fs.write_file(std::path::Path::new("v.rs"), b"pub fn v() -> u32 { 1 }\n")
        .await
        .unwrap();
    let task_id = TaskId::new();
    let mission_id = MissionId::new();
    let mk_proposal = || {
        ChangeProposal::new(
            task_id,
            mission_id,
            ImplementationHypothesis::new("bump", "one", "bump", "two", "t"),
            ChangeSurface::new(vec!["v.rs".to_string()]),
            vec![FileMutationProposal::new(
                "v.rs",
                FileMutationOp::Substring {
                    old_content: "{ 1 }".to_string(),
                    new_content: "{ 2 }".to_string(),
                },
                "bump",
            )],
        )
    };
    let authority = ChangeAuthority::new().with_pool(pool.clone());
    let first = authority
        .execute_change_proposal(ws, &mk_proposal(), &fs, None, None)
        .await
        .expect("first applies");
    assert_eq!(
        tokio::fs::read_to_string(ws.join("v.rs")).await.unwrap(),
        "pub fn v() -> u32 { 2 }\n"
    );
    // Crash replay: identical proposal returns the recorded outcome.
    let second = authority
        .execute_change_proposal(ws, &mk_proposal(), &fs, None, None)
        .await
        .expect("duplicate replays recorded outcome");
    assert_eq!(second.files_modified, first.files_modified);
    assert_eq!(
        tokio::fs::read_to_string(ws.join("v.rs")).await.unwrap(),
        "pub fn v() -> u32 { 2 }\n",
        "no duplicate side effect"
    );
    // Drift after commit: replay detects the workspace no longer holds the
    // recorded result and re-executes honestly (reconcile judges live state).
    tokio::fs::write(ws.join("v.rs"), b"pub fn v() -> u32 { 9 }\n")
        .await
        .unwrap();
    assert!(
        authority
            .execute_change_proposal(ws, &mk_proposal(), &fs, None, None)
            .await
            .is_err(),
        "drifted workspace must not replay a stale outcome"
    );
}

#[tokio::test]
async fn p44_checkpoint_partial_claim_rejected() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let mgr = m31a::checkpoint::manager::CheckpointManager::new(
        pool.clone(),
        store,
        dir.path().join("staging"),
    );
    let ghost = ArtifactId::new();
    let manifest = CheckpointManifest::new(
        CheckpointId::new(),
        mission_id,
        1,
        "Observe",
        1,
        "snap",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy",
        vec![],
        vec![(ghost, "deadbeef".to_string(), 10)],
        "partial claim",
    );
    let err = mgr
        .create_checkpoint(&manifest, vec![])
        .await
        .expect_err("manifest claiming an absent payload must fail");
    assert!(err.to_string().contains(&ghost.to_string()));
}

#[tokio::test]
async fn p44_checkpoint_promotion_conflict_fails_closed() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let mgr = m31a::checkpoint::manager::CheckpointManager::new(
        pool.clone(),
        store,
        dir.path().join("staging"),
    );
    let aid = ArtifactId::new();
    let data1 = b"evidence version one".to_vec();
    let h1 = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&data1);
        format!("{:x}", h.finalize())
    };
    let mk = |data: Vec<u8>, hash: String| {
        CheckpointManifest::new(
            CheckpointId::new(),
            mission_id,
            1,
            "Observe",
            1,
            "snap",
            BTreeMap::new(),
            BTreeMap::new(),
            "policy",
            vec![],
            vec![(aid, hash, data.len() as u64)],
            "conflict probe",
        )
    };
    mgr.create_checkpoint(
        &mk(data1.clone(), h1),
        vec![m31a::checkpoint::manager::StagedArtifactInput::new(
            aid, data1, "bin", "log",
        )],
    )
    .await
    .expect("first promotion records evidence");
    // Same identity, different content: silent overwrite refused.
    let data2 = b"evidence version TWO".to_vec();
    let h2 = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&data2);
        format!("{:x}", h.finalize())
    };
    let err = mgr
        .create_checkpoint(
            &mk(data2.clone(), h2),
            vec![m31a::checkpoint::manager::StagedArtifactInput::new(
                aid, data2, "bin", "log",
            )],
        )
        .await
        .expect_err("conflicting content under one identity must fail");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn p44_checkpoint_orphan_payloads_reclaimed() {
    let (dir, pool, _bus) = setup_test_db().await;
    let (_mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let artifacts_dir = dir.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).unwrap();
    let store = Arc::new(FsArtifactStore::new(&artifacts_dir));
    let mgr = m31a::checkpoint::manager::CheckpointManager::new(
        pool.clone(),
        store,
        dir.path().join("staging"),
    );
    // Simulate a crash between promotion and commit: payload without metadata.
    let orphan = ArtifactId::new();
    std::fs::write(artifacts_dir.join(format!("{orphan}.bin")), b"orphan").unwrap();
    let reclaimed = mgr.gc_orphan_payloads().await.expect("gc runs");
    assert_eq!(reclaimed, 1);
    assert!(!artifacts_dir.join(format!("{orphan}.bin")).exists());
    assert_eq!(mgr.gc_orphan_payloads().await.unwrap(), 0);
}

#[tokio::test]
async fn p44_artifact_service_immutability() {
    let (dir, pool, _bus) = setup_test_db().await;
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let svc = m31a::persistence::artifacts::service::ArtifactService::new(store, pool);
    let id = ArtifactId::new();
    let prov = m31a::persistence::artifacts::service::ArtifactProvenance::new();
    svc.create_and_store_with_id(id, "a", b"content-one", "bin", prov.clone())
        .await
        .expect("first store");
    // Identical content: idempotent, same record.
    let again = svc
        .create_and_store_with_id(id, "a", b"content-one", "bin", prov.clone())
        .await
        .expect("identical rewrite is idempotent");
    assert_eq!(again.id, id);
    // Different content: hard violation, original preserved.
    let err = svc
        .create_and_store_with_id(id, "a", b"content-TWO", "bin", prov)
        .await
        .expect_err("overwrite must fail");
    assert!(err.to_string().contains(&id.to_string()));
}

#[tokio::test]
async fn p44_budget_survives_restart() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let enforcer = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(10_000),
        ..Default::default()
    });
    let receipt = enforcer
        .reserve(
            &TaskEstimates {
                estimated_tokens: 1000,
                ..Default::default()
            },
            true,
        )
        .unwrap();
    enforcer.settle_model_usage(
        &receipt,
        &TokenUsage::new(300, 200, 500, 0, UsageSource::AuthoritativeProvider),
    );
    m31a::budget::BudgetLedger::new(pool.clone())
        .record(mission_id, &enforcer)
        .await
        .expect("ledger records");
    // Simulate restart: fresh enforcer hydrates identical consumption.
    let fresh = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(10_000),
        ..Default::default()
    });
    assert!(
        m31a::budget::BudgetLedger::new(pool.clone())
            .hydrate(mission_id, &fresh)
            .await
            .unwrap(),
        "ledger row must exist"
    );
    assert_eq!(fresh.total_tokens_consumed(), 500);
    // Overrun state survives too: consumption beyond a tighter limit closes
    // admission on the restarted enforcer.
    let tight = BudgetEnforcer::new(ResourceBudget {
        max_tokens: Some(400),
        ..Default::default()
    });
    m31a::budget::BudgetLedger::new(pool.clone())
        .hydrate(mission_id, &tight)
        .await
        .unwrap();
    assert!(
        tight
            .reserve(
                &TaskEstimates {
                    estimated_tokens: 1,
                    ..Default::default()
                },
                true
            )
            .is_err(),
        "restarted enforcer must honor hydrated overrun"
    );
}

#[tokio::test]
async fn p44_recovery_fingerprints_survive_restart() {
    use m31a::recovery::budget::SemanticFailureSignature;
    use m31a::recovery::budget::{RecoveryAttemptRecord, RecoveryBudgetTracker};
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let tracker = RecoveryBudgetTracker::default();
    let sig = SemanticFailureSignature::new(
        FailureClassification::Compilation,
        "src/lib.rs",
        "E0308",
        None,
    );
    tracker
        .record_attempt(
            &pool,
            RecoveryAttemptRecord {
                mission_id,
                task_id,
                failure_class: FailureClassification::Compilation,
                strategy: "retry".to_string(),
                attempt_number: 1,
                budget_consumed: 10,
                remaining_class_budget: 2,
                remaining_overall_budget: 9,
                backoff_delay_ms: 100,
                action_taken: "retry".to_string(),
                result: "failed".to_string(),
                mutation_fingerprint: Some("fp-abc-123".to_string()),
                semantic_signature: Some(serde_json::to_string(&sig).unwrap()),
            },
        )
        .await
        .unwrap();
    // Restart: a hydrated tracker recognizes the identical attempt.
    let restarted = RecoveryBudgetTracker::hydrated(&pool, mission_id)
        .await
        .expect("hydration runs");
    assert!(matches!(
        restarted.classify_attempt_with_mission(Some(mission_id), task_id, "fp-abc-123"),
        m31a::recovery::budget::AttemptStrategyClassification::RepeatedIdentical
    ));
    assert!(matches!(
        restarted.classify_attempt_with_mission(Some(mission_id), task_id, "fp-other"),
        m31a::recovery::budget::AttemptStrategyClassification::ModifiedAttempt
    ));
}

// ===========================================================================
// DATABASE ADVERSARIAL (§22) + FAIL-CLOSED DOCTRINE (§18/§19)
// ===========================================================================

#[tokio::test]
async fn p44_db_corrupt_author_fails_closed() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, _) = create_test_session(&pool, dir.path()).await;
    let coordinator = coordinator_for(&pool, &bus, dir.path());
    coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .unwrap();
    sqlx::query("UPDATE plan_revisions SET author_type = 'ghost_writer'")
        .execute(&pool)
        .await
        .unwrap();
    let repo = SqliteLifecycleRepository::new(pool.clone());
    let err = repo
        .load_plan_revisions(&session_id)
        .await
        .expect_err("corrupt author must fail closed");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
}

#[tokio::test]
async fn p44_db_corrupt_plan_status_fails_closed() {
    let (dir, pool, bus) = setup_test_db().await;
    let (session_id, _) = create_test_session(&pool, dir.path()).await;
    let coordinator = coordinator_for(&pool, &bus, dir.path());
    coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .unwrap();
    sqlx::query("UPDATE plan_revisions SET status = 'approved_by_nobody'")
        .execute(&pool)
        .await
        .unwrap();
    let repo = SqliteLifecycleRepository::new(pool.clone());
    let err = repo
        .load_plan_revisions(&session_id)
        .await
        .expect_err("corrupt status must fail closed");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
}

#[tokio::test]
async fn p44_db_corrupt_failure_class_is_non_retryable() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, task_id, _) = seed_mission_task_agent(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO recovery_attempts (id, mission_id, task_id, failure_class, strategy, attempt_number, budget_consumed, remaining_class_budget, remaining_overall_budget, backoff_delay_ms, action_taken, result, created_at) VALUES (?, ?, ?, 'bogus_xyz', 's', 1, 0, 0, 0, 0, 'a', 'r', ?)",
    )
    .bind(uuid::Uuid::now_v7().as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let records =
        m31a::recovery::budget::RecoveryBudgetTracker::get_task_attempt_records(&pool, task_id)
            .await
            .expect("load runs");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].failure_class, FailureClassification::Corrupt);
    assert!(!records[0].failure_class.is_retryable());
    assert_eq!(records[0].failure_class.default_retry_limit(), 0);
}

#[tokio::test]
async fn p44_db_malformed_limits_never_unlimited() {
    let (_dir, pool, _bus) = setup_test_db().await;
    let (mission_id, _, _) = seed_mission_task_agent(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO budget_grants (grant_id, mission_id, actor, reason, limits_json, granted_at) VALUES ('g1', ?, 'tester', 'probe', '{', ?)",
    )
    .bind(mission_id.to_string())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let repo = m31a::persistence::sqlite::repositories::SqliteBudgetRepository::new(pool.clone());
    let err = repo
        .get_grants(&mission_id)
        .await
        .expect_err("malformed limits must fail closed, never unlimited");
    assert!(err.to_string().contains("corrupt"), "got: {err}");
}

#[tokio::test]
async fn p44_db_interrupted_transition_leaves_no_partial_state() {
    let (dir, pool, bus) = setup_test_db().await;
    let mission_id = MissionId::new();
    let mut deps = ControllerDependencies::production(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a").join("storage"),
        Some(bus.clone()),
    );
    deps = deps.with_budget_enforcer(Arc::new(BudgetEnforcer::new(ResourceBudget::unbounded())));
    deps = deps.with_transaction_manager(Arc::new(
        m31a::persistence::sqlite::transaction::SqliteTransactionManager::new(pool.clone()),
    ));
    // Seed the mission row only: the task row is absent (crash analogue).
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("m").bind("active").bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        bus.clone(),
        CancellationToken::new(),
    );
    let ghost_task = TaskId::new();
    controller.active_task = Some(m31a::kernel::seams::scheduler::WorkItem {
        task_id: ghost_task,
        title: "ghost".to_string(),
        estimated_tokens: 10,
        required_capabilities: vec![],
        description: None,
        completion_criteria: vec![],
        requirement_keys: vec![],
        assumptions: vec![],
        verification: None,
    
        prompt_ref: None,});
    controller.last_execution_result = Some(
        m31a::kernel::seams::execution::WorkExecutionResult::new(
            ghost_task,
            true,
            "out".to_string(),
            None,
        )
        .with_token_usage(TokenUsage::new(
            5,
            5,
            10,
            0,
            UsageSource::AuthoritativeProvider,
        )),
    );
    controller.progress.current_stage = LoopStage::UpdateState;
    let err = controller
        .step()
        .await
        .expect_err("absent task must fail the transition");
    assert!(
        err.to_string().contains("update_state_persistence"),
        "runtime must know persistence failed, got: {err}"
    );
    // Atomicity: no invocation row without its task row (no partial state).
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_invocations WHERE task_id = ?")
        .bind(ghost_task.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "failed transaction must leave no partial telemetry");
}

// ===========================================================================
// TELEMETRY DURABILITY (§20): fixes preserve dimensions, never secrets
// ===========================================================================

#[tokio::test]
async fn p44_telemetry_dimensions_survive_hardening() {
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
        "p44-telemetry",
    )
    .await
    .expect("record");
    assert_eq!(record.mission_id, mission_id);
    assert_eq!(record.task_id, task_id);
    assert_eq!(record.provider, "nvidia");
    assert_eq!(record.total_tokens, 15);
    assert_eq!(record.usage_source, "authoritative_provider");
    // Fence rows carry hashes only — never secrets or prompts.
    let fence_sql: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name = 'tool_mutation_fence'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!fence_sql.to_lowercase().contains("secret"));
    assert!(!fence_sql.to_lowercase().contains("prompt"));
}

// ===========================================================================
// LIVE SECURITY PROBE (§23, #[ignore])
// ===========================================================================

/// Controlled live probe: real model → real tool path → scrubbed execution →
/// real accounting. Small and bounded by construction.
#[tokio::test]
#[ignore = "requires live NVIDIA credentials + WAN; run explicitly with --ignored"]
async fn live_p44_security_probe_model_tool_accounting() {
    use m31a::agent::model_policy::ModelCaller;
    let harness = match m31a::testing::real_model::RealModelHarness::ensure() {
        Ok(h) => h,
        Err(skipped) => {
            eprintln!("LIVE SKIPPED (environment): {}", skipped.reason);
            return;
        }
    };
    // Real model with authoritative usage.
    let caller = harness.routed_caller(Vec::new());
    let token = CancellationToken::new();
    let (proposal, usage) = caller
        .call_model_cancellable_with_usage("Return the text: 'p44 live security ok'", &token)
        .await
        .expect("live model call");
    assert!(
        proposal.is_completion()
            || matches!(
                proposal,
                m31a::model::types::ModelProposal::AssistantText { .. }
            )
    );
    assert_eq!(usage.source, UsageSource::AuthoritativeProvider);

    // Real hardened execution: scrubbed env, contained cwd, bounded output.
    let dir = tempdir().unwrap();
    let spawn = HardenedSpawn::new(dir.path())
        .with_env_var("M44_LIVE", "live-ok")
        .unwrap();
    let mut cmd = spawn
        .build_command(
            "sh",
            &["-c".to_string(), "echo $M44_LIVE".to_string()],
            None,
        )
        .expect("hardened build");
    let out = tokio::time::timeout(Duration::from_secs(30), cmd.output())
        .await
        .expect("bounded")
        .expect("spawn");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "live-ok");

    // Real accounting settlement with zero leakage.
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
