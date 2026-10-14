//! Second-stage remediation regression suite (October 14, 2026 audit follow-up).
//!
//! Every test proves a behavioral invariant on the REAL production path —
//! never a mocked reconstruction, never a bare `Ok(())` assertion:
//!
//! - NEW-01: broad `.tmp*` risk exemption removed; sensitive files keep
//!   classification under lexical and symlink aliases.
//! - F-02: `execution_runs` is the sole crash-recovery authority; stale
//!   legacy `missions` rows never resurrect terminal runs.
//! - F-03/F-12: real pause protocol — quiescence acknowledgement, timeout
//!   truthfulness, no false events, no dropped in-flight work.
//! - F-04/F-05: canonical resume — worktree scope restoration, evidence
//!   validation, no CLI-fabricated state.
//! - F-06: lifecycle transition contract under concurrency.
//! - F-07: approval waiter lifecycle with deterministic synchronization.
//! - F-08: symlink-safe, completeness-honest baseline capture.
//! - F-09: authorized resource binding enforced through to mutation.
//! - F-10: mandatory task-output integrity evidence on recovery.
//! - F-11: integrity check still runs on empty recovery scans.
//! - F-13: per-execution approval configuration isolation.
//! - F-14: truthful workspace fingerprints (`unchanged`/`changed`/`unknown`).
//! - F-15: authorization ordering with staged rollback.
//! - F-16: prior-pass preservation guards (unicode-safe redaction).
//! - F-17: exact file-content fidelity.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::Barrier;
use tokio_util::sync::CancellationToken;

use m31a::agent::adaptive::{ExecutionObservation, FingerprintComparison, StallEvaluation};
use m31a::agent::adaptive::{
    StallDetector, UNKNOWN_FINGERPRINT_PREFIX, compare_workspace_fingerprints,
};
use m31a::capability::providers::local_fs::LocalFileSystemProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::capability::traits::fs::FileSystemService;
use m31a::checkpoint::crash_recovery::{
    CrashRecoveryClassification, CrashRecoveryError, StartupCrashRecoveryScanner,
};
use m31a::checkpoint::manager::{CheckpointManager, StagedArtifactInput};
use m31a::checkpoint::manifest::{CURRENT_CHECKPOINT_SCHEMA_VERSION, CheckpointManifest};
use m31a::checkpoint::resume::SafeResumeEngine;
use m31a::cli::{CliDispatcher, RuntimeCommand};
use m31a::error::M31AError;
use m31a::events::EventType;
use m31a::events::bus::{EventBus, EventFilter};
use m31a::ids::{
    ApprovalRequestId, ArtifactId, CheckId, CheckpointId, ExecutionRunId, MissionId, TaskId,
    ToolCallId,
};
use m31a::kernel::plan::{CandidatePlan, CandidateTask, ResourceEstimate, VerificationStrategy};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::persistence::sqlite::repositories::{
    PersistedLifecycleState, SqliteExecutionRunRepository,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::stages::{ResourceBinding, ResourceScopeStage};
use m31a::policy::approval::coordinator::ApprovalCoordinator;
use m31a::policy::approval::{
    ApprovalAction, ApprovalChannel, ApprovalError, ApprovalExplanationPacket, ApprovalRequest,
};
use m31a::repo::drift::{RepositoryBaseline, detect_drift};
use m31a::runtime::{AppRuntime, PlanningChoice};
use m31a::state::PolicyContext;
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::{ExecutionRunState, TaskState};
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::fs::{EditFileInput, EditFileTool, WriteFileInput, WriteFileTool};
use m31a::tools::risk::{EffectiveRisk, RiskClass, is_sensitive_path};

// ============================================================================
// Shared doubles & helpers
// ============================================================================

fn now_str() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

#[derive(Default, Clone)]
struct MockApprovalChannel {
    notified: Arc<tokio::sync::Mutex<Vec<ApprovalExplanationPacket>>>,
}

#[async_trait]
impl ApprovalChannel for MockApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        self.notified.lock().await.push(packet.clone());
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

#[derive(Clone)]
struct FailingApprovalChannel;

#[async_trait]
impl ApprovalChannel for FailingApprovalChannel {
    async fn notify_request(
        &self,
        _packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        Err(ApprovalError::ChannelClosed(
            "operator channel unreachable".to_string(),
        ))
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

/// Shared handle letting an [`ImmediateChannel`] resolve through the live coordinator.
type CoordinatorHolder = Arc<tokio::sync::RwLock<Option<Arc<ApprovalCoordinator>>>>;

/// Channel resolving every request with a fixed action inside `notify_request`.
#[derive(Clone)]
struct ImmediateChannel {
    action: ApprovalAction,
    coordinator: CoordinatorHolder,
}

#[async_trait]
impl ApprovalChannel for ImmediateChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        if let Some(coord) = self.coordinator.read().await.as_ref() {
            coord
                .resolve_request(packet.request_id, self.action.clone(), "immediate_operator")
                .await?;
        }
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

fn immediate_coordinator(
    pool: Option<sqlx::SqlitePool>,
    action: ApprovalAction,
) -> (Arc<ApprovalCoordinator>, CoordinatorHolder) {
    let handle: CoordinatorHolder = Arc::new(tokio::sync::RwLock::new(None));
    let channel = ImmediateChannel {
        action,
        coordinator: handle.clone(),
    };
    let coord = Arc::new(ApprovalCoordinator::new(
        pool,
        Some(Arc::new(channel) as Arc<dyn ApprovalChannel>),
    ));
    (coord, handle)
}

async fn set_channel_holder(handle: &CoordinatorHolder, coord: &Arc<ApprovalCoordinator>) {
    *handle.write().await = Some(coord.clone());
}

fn make_approval_request(mission: MissionId, task: Option<TaskId>) -> ApprovalRequest {
    ApprovalRequest::new(
        mission,
        task,
        None,
        ToolCallId::new(),
        "write_file",
        serde_json::json!({"path": "notes.txt"}),
        vec!["notes.txt".to_string()],
        RiskClass::HighRiskMutation,
        None,
        "policy-hash-1",
        "operator sign-off required",
    )
}

/// Seed the durable parent rows an approval request's foreign keys require
/// (missions + tasks). Without them the request insert fails closed on FK.
async fn seed_approval_scope(pool: &sqlx::SqlitePool, mission: MissionId, task: TaskId) {
    seed_legacy_mission(pool, mission, "running").await;
    let now = now_str();
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind("approval task")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .expect("seed task for approval");
}

#[derive(Debug, Clone)]
struct LoopingToolCaller {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl m31a::agent::model_policy::ModelCaller for LoopingToolCaller {
    async fn call_model(&self, context: &str) -> Result<m31a::model::types::ModelProposal, String> {
        if context.contains("planning.decompose")
            || context.contains("Lead Planner")
            || context.contains("Task Decomposition")
        {
            let decomp = r#"{
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Loop reading probe file",
                        "description": "Read task",
                        "depends_on": [],
                        "role": "researcher",
                        "required_capabilities": ["repo.read", "fs.read"],
                        "verification": "artifact_inspection"
                    }
                ]
            }"#;
            return Ok(m31a::model::types::ModelProposal::Complete {
                summary: decomp.to_string(),
                artifacts: Vec::new(),
            });
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(m31a::model::types::ModelProposal::ToolCalls {
            calls: vec![m31a::model::types::ModelToolCall::new(
                "read_file",
                serde_json::json!({"path": "probe.txt"}),
            )],
        })
    }
}

#[derive(Debug, Clone, Default)]
struct PendingModelCaller;

#[async_trait]
impl m31a::agent::model_policy::ModelCaller for PendingModelCaller {
    async fn call_model(
        &self,
        _context: &str,
    ) -> Result<m31a::model::types::ModelProposal, String> {
        std::future::pending().await
    }
}

#[derive(Debug, Clone, Default)]
struct CompleteModelCaller;

#[async_trait]
impl m31a::agent::model_policy::ModelCaller for CompleteModelCaller {
    async fn call_model(
        &self,
        _context: &str,
    ) -> Result<m31a::model::types::ModelProposal, String> {
        Ok(m31a::model::types::ModelProposal::Complete {
            summary: "stub work complete".to_string(),
            artifacts: Vec::new(),
        })
    }
}

/// Stub that answers planning decomposition with a valid task graph and
/// completes every other turn. Used where the worker must dispatch and run
/// to a halt without touching manually-seeded tasks.
#[derive(Debug, Clone, Default)]
struct DecomposeAwareCaller;

#[async_trait]
impl m31a::agent::model_policy::ModelCaller for DecomposeAwareCaller {
    async fn call_model(&self, context: &str) -> Result<m31a::model::types::ModelProposal, String> {
        if context.contains("planning.decompose")
            || context.contains("Lead Planner")
            || context.contains("Task Decomposition")
        {
            let decomp = r#"{
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Execute mission objective",
                        "description": "Research task",
                        "depends_on": [],
                        "role": "researcher",
                        "required_capabilities": ["repo.read", "fs.read"],
                        "verification": "artifact_inspection"
                    }
                ]
            }"#;
            return Ok(m31a::model::types::ModelProposal::Complete {
                summary: decomp.to_string(),
                artifacts: Vec::new(),
            });
        }
        Ok(m31a::model::types::ModelProposal::Complete {
            summary: "stub work complete".to_string(),
            artifacts: Vec::new(),
        })
    }
}

async fn setup_runtime_with_caller<C>(dir: &tempfile::TempDir, caller: C) -> Arc<AppRuntime>
where
    C: m31a::agent::model_policy::ModelCaller + 'static,
{
    Arc::new(
        AppRuntime::new(dir.path())
            .await
            .expect("AppRuntime::new")
            .with_model_caller(Arc::new(caller)),
    )
}

async fn seed_execution_run(
    pool: &sqlx::SqlitePool,
    id: ExecutionRunId,
    status: &str,
    workspace: &str,
) {
    let policy_ctx = serde_json::to_string(&PolicyContext::standard()).unwrap();
    let now = now_str();
    sqlx::query(
        r#"
        INSERT INTO execution_runs (
            id, objective, status, mode, workspace_root, policy_context, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.as_bytes().as_slice())
    .bind("remediation test run")
    .bind(status)
    .bind("Safe")
    .bind(workspace)
    .bind(&policy_ctx)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .expect("seed execution_runs");
}

async fn seed_legacy_mission(pool: &sqlx::SqlitePool, id: ExecutionRunId, status: &str) {
    let now = now_str();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id.as_bytes().as_slice())
    .bind("legacy mission row")
    .bind(status)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .expect("seed missions");
}

async fn run_status(pool: &sqlx::SqlitePool, id: ExecutionRunId) -> String {
    sqlx::query_scalar("SELECT status FROM execution_runs WHERE id = ?")
        .bind(id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("read run status")
}

async fn drain_events(
    rx: &mut m31a::events::bus::EventReceiver,
    window: Duration,
) -> Vec<m31a::events::envelope::EventEnvelope> {
    let mut out = Vec::new();
    let deadline = tokio::time::sleep(window);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            item = rx.next() => {
                match item {
                    Some(Ok(env)) => out.push(env),
                    Some(Err(_)) => break,
                    None => break,
                }
            }
        }
    }
    out
}

fn count_events(
    envs: &[m31a::events::envelope::EventEnvelope],
    run: ExecutionRunId,
    name: &str,
) -> usize {
    envs.iter()
        .filter(|e| {
            let (rid, ev_name) = match &e.event_type {
                EventType::MissionPaused { mission_id, .. } => (*mission_id, "paused"),
                EventType::MissionResumed { mission_id } => (*mission_id, "resumed"),
                EventType::MissionCompleted { mission_id } => (*mission_id, "completed"),
                EventType::MissionFailed { mission_id, .. } => (*mission_id, "failed"),
                EventType::MissionCancelled { mission_id, .. } => (*mission_id, "cancelled"),
                _ => return false,
            };
            rid == run && ev_name == name
        })
        .count()
}

// ============================================================================
// NEW-01: broad `.tmp*` exemption removed
// ============================================================================

#[test]
fn new01_tmp_prefixed_names_are_sensitive() {
    for name in [
        ".tmp.env",
        ".tmpsecrets",
        ".tmpcredentials",
        ".tmp-private-key",
        ".tmpdir/.ssh/id_ed25519",
        "sub/.tmpkey.pem",
    ] {
        assert!(
            is_sensitive_path(Path::new(name)),
            "'{name}' must be classified sensitive"
        );
        let risk = EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new(name));
        assert_eq!(
            risk.effective_risk(),
            RiskClass::HighRiskMutation,
            "'{name}' must escalate to HighRiskMutation"
        );
    }
}

#[test]
fn new01_broad_tmp_exemption_regression_guard() {
    // Any dotfile starting with `.tmp` is sensitive. If the old
    // `starts_with(".tmp") → continue` exemption is reintroduced, these fail.
    for name in [".tmpfoo", ".tmp_anything", ".tmpx", ".tmp"] {
        assert!(
            is_sensitive_path(Path::new(name)),
            "dotfile '{name}' must stay sensitive (no .tmp* exemption)"
        );
    }
    // Non-sensitive controls still classify low-risk.
    let benign =
        EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new("src/scratch.txt"));
    assert_eq!(benign.effective_risk(), RiskClass::LowRiskMutation);
}

#[test]
fn new01_runtime_managed_temp_benign_file_allowed() {
    let sys_tmp = std::env::temp_dir();
    let benign = sys_tmp.join(format!(
        "m31a-remediation-bening-{}.txt",
        std::process::id()
    ));
    std::fs::write(&benign, b"benign temp content").unwrap();
    assert!(
        !is_sensitive_path(&benign),
        "benign runtime-managed temp file must not be sensitive"
    );
    let _ = std::fs::remove_file(&benign);
}

#[test]
fn new01_sensitive_alias_inside_managed_temp_stays_sensitive() {
    let sys_tmp = std::env::temp_dir();
    // Lexical alias inside the managed namespace that names a sensitive file.
    let alias = sys_tmp.join(".tmp.env");
    assert!(
        is_sensitive_path(&alias),
        "managed-temp alias '.tmp.env' must stay sensitive"
    );
    let key_alias = sys_tmp.join(".ssh").join("id_ed25519");
    assert!(
        is_sensitive_path(&key_alias),
        "managed-temp alias '.ssh/id_ed25519' must stay sensitive"
    );
}

#[cfg(unix)]
#[test]
fn new01_symlink_alias_to_sensitive_escalates() {
    let outer = tempdir().unwrap();
    let secret = outer.path().join("real_secret.env");
    std::fs::write(&secret, b"SECRET=1").unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let link = ws.join(".tmp_link_env");
    std::os::unix::fs::symlink(&secret, &link).unwrap();

    // Effective-target evaluation sees through the alias.
    let effective = link.canonicalize().ok();
    let risk = EffectiveRisk::evaluate_file_target(
        RiskClass::LowRiskMutation,
        Path::new(".tmp_link_env"),
        effective.as_deref(),
    );
    assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);
}

// ============================================================================
// F-09: authorized resource binding through to mutation
// ============================================================================

#[tokio::test]
async fn f09_write_through_symlink_refused_and_real_path_allowed() {
    let dir = tempdir().unwrap();
    let provider = LocalFileSystemProvider::new(dir.path()).unwrap();
    let target = dir.path().join("real.txt");
    tokio::fs::write(&target, b"original").await.unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("real.txt", dir.path().join("alias.txt")).unwrap();
        let err = provider
            .write_file(Path::new("alias.txt"), b"hijacked")
            .await
            .expect_err("mutation through symlink alias must be refused");
        assert!(
            err.to_string().contains("symlink"),
            "unexpected error: {err}"
        );
        // Target untouched.
        let content = tokio::fs::read(&target).await.unwrap();
        assert_eq!(content, b"original");
    }
    // Real path still works.
    provider
        .write_file(Path::new("real.txt"), b"updated")
        .await
        .expect("real-path write must succeed");
    let content = tokio::fs::read(&target).await.unwrap();
    assert_eq!(content, b"updated");
}

#[tokio::test]
async fn f09_binding_swap_detected_before_mutation() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let file_a = ws.join("a.txt");
    let file_b = ws.join("b.txt");
    std::fs::write(&file_a, b"A").unwrap();
    std::fs::write(&file_b, b"B").unwrap();
    #[cfg(unix)]
    {
        let link = ws.join("link.txt");
        std::os::unix::fs::symlink("a.txt", &link).unwrap();
        // Authorize binding link.txt -> a.txt (workspace-relative effective).
        let authorized = ResourceBinding {
            lexical: PathBuf::from("link.txt"),
            effective: PathBuf::from("a.txt"),
        };
        ResourceScopeStage::verify_bindings_live(std::slice::from_ref(&authorized), &ws)
            .expect("matching binding must verify");
        // Attacker swaps the symlink between authorization and execution.
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("b.txt", &link).unwrap();
        let err = ResourceScopeStage::verify_bindings_live(std::slice::from_ref(&authorized), &ws)
            .expect_err("swapped binding must be rejected");
        assert!(
            err.to_string().contains("RESOURCE_BINDING_MISMATCH"),
            "unexpected error: {err}"
        );
    }
}

#[tokio::test]
async fn f09_delete_symlink_alias_refused() {
    let dir = tempdir().unwrap();
    let provider = LocalFileSystemProvider::new(dir.path()).unwrap();
    let target = dir.path().join("victim.txt");
    tokio::fs::write(&target, b"precious").await.unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("victim.txt", dir.path().join("alias.txt")).unwrap();
        let err = provider
            .delete_file(Path::new("alias.txt"))
            .await
            .expect_err("deleting a symlink alias must be refused");
        assert!(
            err.to_string().contains("symlink"),
            "unexpected error: {err}"
        );
        assert!(target.exists(), "link target must survive refused delete");
    }
}

// ============================================================================
// F-02: execution_runs is the sole crash-recovery authority (+ F-11 guard)
// ============================================================================

fn f02_scanner(pool: &sqlx::SqlitePool, workspace: &Path) -> StartupCrashRecoveryScanner {
    let store: Arc<dyn ArtifactStore> =
        Arc::new(FsArtifactStore::new(workspace.join(".m31a-artifacts")));
    StartupCrashRecoveryScanner::new(pool.clone(), store, workspace)
}

#[tokio::test]
async fn f02_canonical_active_run_recovered() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Executing", &dir.path().to_string_lossy()).await;

    let scanner = f02_scanner(&pool, dir.path());
    let results = scanner.scan_all_in_flight().await.expect("scan");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].mission_id, run);
    assert_eq!(
        results[0].classification,
        CrashRecoveryClassification::SafeToResume
    );
}

#[tokio::test]
async fn f02_completed_with_stale_legacy_not_resurrected() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Completed", &dir.path().to_string_lossy()).await;
    seed_legacy_mission(&pool, run, "in_progress").await;

    let scanner = f02_scanner(&pool, dir.path());
    let results = scanner.scan_all_in_flight().await.expect("scan");
    assert!(
        results.is_empty(),
        "canonically completed run must not be recovered"
    );
    let direct = scanner.scan_and_reconcile(run).await;
    assert!(
        matches!(direct, Err(CrashRecoveryError::AlreadySettled(_))),
        "direct scan of settled run must fail closed, got: {direct:?}"
    );
    assert_eq!(run_status(&pool, run).await, "Completed");
}

#[tokio::test]
async fn f02_cancelled_with_stale_legacy_not_resurrected() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Cancelled", &dir.path().to_string_lossy()).await;
    seed_legacy_mission(&pool, run, "running").await;

    let scanner = f02_scanner(&pool, dir.path());
    let results = scanner.scan_all_in_flight().await.expect("scan");
    assert!(
        results.is_empty(),
        "canonically cancelled run must not be recovered"
    );
    assert_eq!(run_status(&pool, run).await, "Cancelled");
}

#[tokio::test]
async fn f02_legacy_only_install_migrated_once() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_legacy_mission(&pool, run, "in_progress").await;

    let scanner = f02_scanner(&pool, dir.path());
    let results = scanner.scan_all_in_flight().await.expect("scan");
    assert_eq!(results.len(), 1, "genuine legacy row must be reconciled");
    assert_eq!(
        results[0].classification,
        CrashRecoveryClassification::SafeToResume
    );
    // Canonical row now exists with mapped status.
    assert_eq!(run_status(&pool, run).await, "Executing");

    // Second scan: no duplicate canonical row, same single result.
    let again = scanner.scan_all_in_flight().await.expect("rescan");
    assert_eq!(again.len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs WHERE id = ?")
        .bind(run.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "reconciliation must not duplicate canonical rows");
}

#[tokio::test]
async fn f02_repeated_scans_idempotent() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Executing", &dir.path().to_string_lossy()).await;

    let scanner = f02_scanner(&pool, dir.path());
    let first = scanner.scan_all_in_flight().await.expect("scan 1");
    let second = scanner.scan_all_in_flight().await.expect("scan 2");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].classification, second[0].classification);
    assert_eq!(run_status(&pool, run).await, "Executing");
}

#[tokio::test]
async fn f11_empty_scan_still_checks_integrity() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f11.db"))
        .await
        .unwrap();
    let scanner = f02_scanner(&pool, dir.path());
    let results = scanner.scan_all_in_flight().await.expect("empty scan");
    assert!(results.is_empty());
    assert_eq!(
        scanner.integrity_check_count(),
        1,
        "database integrity must be verified even with zero in-flight runs"
    );
}

#[tokio::test]
async fn f02_session_insert_reconciles_legacy_alias() {
    use m31a::persistence::sqlite::repositories::{SessionRepository, SqliteSessionRepository};

    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    // Legacy-only installation: missions row, no canonical row.
    let mission = MissionId::new();
    seed_legacy_mission(&pool, mission, "in_progress").await;

    // Legacy session creation aliases the mission id as the run id. The
    // session boundary must reconcile it (create-if-absent) rather than
    // fail or fabricate lifecycle state.
    let session_repo = SqliteSessionRepository::new(pool.clone());
    let session = m31a::state::session::Session::for_mission(m31a::ids::SessionId::new(), mission);
    session_repo
        .insert(&session)
        .await
        .expect("legacy session insert must reconcile");

    // Canonical row minted with mapped status; terminal rows never minted here.
    assert_eq!(run_status(&pool, mission).await, "Executing");
    let listed = session_repo.list_by_run(mission).await.unwrap();
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn f02_session_insert_dangling_pointer_fails_closed() {
    use m31a::persistence::sqlite::repositories::{SessionRepository, SqliteSessionRepository};

    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let session_repo = SqliteSessionRepository::new(pool);
    // Points at an id present in NEITHER table: must fail, never fabricate.
    let session =
        m31a::state::session::Session::for_mission(m31a::ids::SessionId::new(), MissionId::new());
    assert!(
        session_repo.insert(&session).await.is_err(),
        "dangling session association must fail closed"
    );
}

#[tokio::test]
async fn f02_legacy_mapping_rejects_unknown_status() {
    use m31a::persistence::sqlite::repositories::{SessionRepository, SqliteSessionRepository};

    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f02.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    seed_legacy_mission(&pool, mission, "Nonsense").await;

    let session_repo = SqliteSessionRepository::new(pool.clone());
    let session = m31a::state::session::Session::for_mission(m31a::ids::SessionId::new(), mission);
    let err = session_repo
        .insert(&session)
        .await
        .expect_err("unknown legacy status must fail closed, not mint canonical state");
    assert!(
        matches!(
            err,
            M31AError::ValidationError(_) | M31AError::PersistenceError(_)
        ),
        "got: {err:?}"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "no canonical row may be minted from unknown vocabulary"
    );
}

// ============================================================================
// F-03/F-12: real pause protocol against live workers
// ============================================================================

/// Spawn a genuinely-working execution (infinite read loop) and return its run id.
async fn spawn_looping_execution(
    runtime: &Arc<AppRuntime>,
) -> (
    ExecutionRunId,
    tokio::task::JoinHandle<Result<m31a::runtime::ExecutionSummary, M31AError>>,
) {
    let rt = runtime.clone();
    let handle = tokio::spawn(async move {
        rt.run_execution(
            "loop reading probe file",
            None,
            true,
            Some(PlanningChoice::DirectExecution),
        )
        .await
    });
    // Discover the run id and wait for it to become Executing.
    let pool = runtime.pool().clone();
    let start = std::time::Instant::now();
    let run = loop {
        let row: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT id FROM execution_runs ORDER BY created_at DESC LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap();
        if let Some(bytes) = row {
            assert!(bytes.len() == 16);
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&bytes);
            let id = ExecutionRunId::from_bytes(arr);
            if run_status(&pool, id).await == "Executing" {
                break id;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "worker never reached Executing"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    (run, handle)
}

#[tokio::test]
async fn f03_active_worker_pause_quiesces() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), b"probe content\n").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = setup_runtime_with_caller(
        &dir,
        LoopingToolCaller {
            calls: calls.clone(),
        },
    )
    .await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    let (run, handle) = spawn_looping_execution(&runtime).await;
    // Wait until the worker is demonstrably mid-loop (real tool calls
    // flowing), so the pause provably lands on an ACTIVE worker.
    let start = std::time::Instant::now();
    loop {
        if calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "worker never started real tool work"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    runtime
        .pause_execution(run, "operator pause")
        .await
        .expect("pause of active worker must succeed");

    // Worker runs to its halt; pausing must not turn it into failure/completion.
    let summary = tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .expect("worker must halt after pause")
        .expect("join")
        .expect("worker halt must be Ok");
    assert_eq!(summary.status, "Paused", "halt summary must be Paused");

    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
    let envs = drain_events(&mut rx, Duration::from_secs(2)).await;
    assert_eq!(
        count_events(&envs, run, "paused"),
        1,
        "exactly one MissionPaused must be emitted (no duplicates)"
    );
    assert_eq!(
        count_events(&envs, run, "failed"),
        0,
        "pause is not failure"
    );
    assert_eq!(
        count_events(&envs, run, "completed"),
        0,
        "pause is not completion"
    );

    // Idempotent repeat.
    runtime
        .pause_execution(run, "duplicate pause")
        .await
        .expect("repeated pause must succeed");
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
}

#[tokio::test]
async fn f03_quiescence_timeout_truthful_error() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), b"probe\n").unwrap();
    let runtime = setup_runtime_with_caller(&dir, PendingModelCaller).await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    let (run, handle) = spawn_looping_execution(&runtime).await;
    let err = runtime
        .pause_execution(run, "pause stuck worker")
        .await
        .expect_err("pause must time out while worker is stuck mid-tick");
    assert!(
        err.to_string().contains("acknowledgement"),
        "timeout error must be actionable, got: {err}"
    );
    // No false success: no event, durable state untouched, worker alive.
    let envs = drain_events(&mut rx, Duration::from_millis(500)).await;
    assert_eq!(
        count_events(&envs, run, "paused"),
        0,
        "timed-out pause must emit no success event"
    );
    assert_eq!(run_status(runtime.pool(), run).await, "Executing");
    handle.abort();
}

#[tokio::test]
async fn f03_pause_persistence_failure_no_false_event() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let run = ExecutionRunId::new();
    seed_execution_run(
        runtime.pool(),
        run,
        "Executing",
        &dir.path().to_string_lossy(),
    )
    .await;
    // Corrupt the status out from under the repository.
    sqlx::query("UPDATE execution_runs SET status = 'Bogus' WHERE id = ?")
        .bind(run.as_bytes().as_slice())
        .execute(runtime.pool())
        .await
        .unwrap();

    let err = runtime
        .pause_execution(run, "pause corrupt run")
        .await
        .expect_err("pause with unreadable state must fail");
    let envs = drain_events(&mut rx, Duration::from_millis(300)).await;
    assert_eq!(
        count_events(&envs, run, "paused"),
        0,
        "failed pause must emit no success event, got err: {err}"
    );
    let raw: String = sqlx::query_scalar("SELECT status FROM execution_runs WHERE id = ?")
        .bind(run.as_bytes().as_slice())
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(raw, "Bogus");
}

#[tokio::test]
async fn f03_pause_terminal_and_non_executing_rejected() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let ws = dir.path().to_string_lossy().to_string();

    let done = ExecutionRunId::new();
    seed_execution_run(runtime.pool(), done, "Completed", &ws).await;
    let err = runtime
        .pause_execution(done, "pause completed")
        .await
        .expect_err("terminal pause must be rejected");
    assert!(matches!(err, M31AError::ValidationError(_)), "got: {err:?}");

    let created = ExecutionRunId::new();
    seed_execution_run(runtime.pool(), created, "Created", &ws).await;
    let err2 = runtime
        .pause_execution(created, "pause created")
        .await
        .expect_err("non-executing pause must be rejected, not fabricated");
    assert!(
        matches!(err2, M31AError::ValidationError(_)),
        "got: {err2:?}"
    );
    // And no false Paused was persisted.
    assert_eq!(run_status(runtime.pool(), created).await, "Created");
}

#[tokio::test]
async fn f03_concurrent_duplicate_pause_single_event() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let run = ExecutionRunId::new();
    seed_execution_run(
        runtime.pool(),
        run,
        "Executing",
        &dir.path().to_string_lossy(),
    )
    .await;

    let barrier = Arc::new(Barrier::new(5));
    let mut handles = Vec::new();
    for _ in 0..5 {
        let rt = runtime.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            rt.pause_execution(run, "concurrent pause").await
        }));
    }
    for h in handles {
        h.await
            .unwrap()
            .expect("concurrent duplicate pause must succeed");
    }
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
    let envs = drain_events(&mut rx, Duration::from_secs(1)).await;
    assert_eq!(
        count_events(&envs, run, "paused"),
        1,
        "concurrent pauses must converge on a single event"
    );
}

// ============================================================================
// F-04/F-05: canonical resume, worktree scope, evidence validation, CLI truth
// ============================================================================

#[tokio::test]
async fn f04_cli_lifecycle_without_runtime_explicit_errors() {
    let dispatcher = CliDispatcher::new();
    let id = ExecutionRunId::new().to_string();

    let err = dispatcher
        .dispatch(RuntimeCommand::PauseMission { id: id.clone() })
        .await
        .expect_err("CLI pause without runtime must fail");
    assert!(
        err.to_string().contains("attached runtime"),
        "pause error must direct to attach a runtime, got: {err}"
    );

    let err = dispatcher
        .dispatch(RuntimeCommand::ResumeMission { id: id.clone() })
        .await
        .expect_err("CLI resume without runtime must fail, never fabricate Executing");
    assert!(
        err.to_string().contains("attached runtime"),
        "resume error must direct to attach a runtime, got: {err}"
    );

    let err = dispatcher
        .dispatch(RuntimeCommand::CancelMission {
            id,
            reason: Some("halt".to_string()),
        })
        .await
        .expect_err("CLI cancel without runtime must fail");
    assert!(
        err.to_string().contains("attached runtime"),
        "cancel error must direct to attach a runtime, got: {err}"
    );
}

#[tokio::test]
async fn f04_resume_missing_worktree_branch_fails_closed() {
    let dir = tempdir().unwrap();
    // Turn the workspace into a real git repository.
    let ws = dir.path();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(ws)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?} failed: {out:?}");
    };
    git(&["init"]);
    git(&["config", "user.email", "test@m31a.local"]);
    git(&["config", "user.name", "m31a-test"]);
    git(&["commit", "--allow-empty", "-m", "init"]);

    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let run = ExecutionRunId::new();
    seed_execution_run(runtime.pool(), run, "Paused", &ws.to_string_lossy()).await;
    // Original worktree branch survives, but the worktree itself is gone.
    git(&["branch", &format!("m31a/{run}")]);

    let err = runtime
        .resume_mission(run)
        .await
        .expect_err("resume with unrecoverable worktree scope must fail closed");
    assert!(
        err.to_string().contains("worktree"),
        "error must name the worktree scope problem, got: {err}"
    );
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
    let envs = drain_events(&mut rx, Duration::from_millis(500)).await;
    assert_eq!(
        count_events(&envs, run, "resumed"),
        0,
        "failed resume must emit no success event"
    );
}

#[tokio::test]
async fn f04_resume_foreign_workspace_refused() {
    let dir = tempdir().unwrap();
    let other = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let run = ExecutionRunId::new();
    seed_execution_run(
        runtime.pool(),
        run,
        "Paused",
        &other.path().to_string_lossy(),
    )
    .await;

    let err = runtime
        .resume_mission(run)
        .await
        .expect_err("cross-workspace resume must be refused");
    assert!(
        err.to_string().contains("workspace"),
        "error must name the workspace binding, got: {err}"
    );
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
}

async fn seed_paused_run_with_checkpoint(
    pool: &sqlx::SqlitePool,
    run: ExecutionRunId,
    ws: &str,
    manifest_json: &str,
    manifest_hash: Option<&str>,
) {
    seed_execution_run(pool, run, "Paused", ws).await;
    let cp = CheckpointId::new();
    let now = now_str();
    sqlx::query(
        r#"
        INSERT INTO checkpoints (
            id, run_id, mission_id, sequence, stage, cycle, state_summary,
            manifest_json, manifest_hash, created_at
        ) VALUES (?, ?, ?, 1, 'Observe', 1, 'summary', ?, ?, ?)
        "#,
    )
    .bind(cp.as_bytes().as_slice())
    .bind(run.as_bytes().as_slice())
    .bind(run.as_bytes().as_slice())
    .bind(manifest_json)
    .bind(manifest_hash)
    .bind(&now)
    .execute(pool)
    .await
    .expect("seed checkpoint");
}

#[tokio::test]
async fn f05_missing_manifest_hash_fails_closed() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let run = ExecutionRunId::new();
    let manifest = CheckpointManifest::new(
        CheckpointId::new(),
        run,
        1,
        "Observe",
        1,
        "snap",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy",
        Vec::new(),
        Vec::new(),
        "no hash checkpoint",
    );
    let json = serde_json::to_string(&manifest).unwrap();
    seed_paused_run_with_checkpoint(
        runtime.pool(),
        run,
        &dir.path().to_string_lossy(),
        &json,
        None,
    )
    .await;

    let err = runtime
        .resume_mission(run)
        .await
        .expect_err("checkpoint without mandatory hash evidence must fail closed");
    assert!(
        matches!(err, M31AError::ReconstructionError(_)),
        "must be an integrity error, got: {err:?}"
    );
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
    let envs = drain_events(&mut rx, Duration::from_millis(300)).await;
    assert_eq!(count_events(&envs, run, "resumed"), 0);
}

#[tokio::test]
async fn f05_legacy_schema_version_rejected() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let run = ExecutionRunId::new();
    let mut manifest = CheckpointManifest::new(
        CheckpointId::new(),
        run,
        1,
        "Observe",
        1,
        "snap",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy",
        Vec::new(),
        Vec::new(),
        "legacy checkpoint",
    );
    assert_eq!(CURRENT_CHECKPOINT_SCHEMA_VERSION, 1);
    manifest.schema_version = 0;
    let json = serde_json::to_string(&manifest).unwrap();
    let hash = manifest.compute_manifest_hash();
    seed_paused_run_with_checkpoint(
        runtime.pool(),
        run,
        &dir.path().to_string_lossy(),
        &json,
        Some(&hash),
    )
    .await;

    let err = runtime
        .resume_mission(run)
        .await
        .expect_err("legacy checkpoint schema must be rejected, not blindly accepted");
    assert!(
        matches!(err, M31AError::ReconstructionError(_)),
        "must be an integrity error, got: {err:?}"
    );
    assert_eq!(run_status(runtime.pool(), run).await, "Paused");
}

// ============================================================================
// F-06: lifecycle transition contract (races, terminal confinement)
// ============================================================================

#[tokio::test]
async fn f06_concurrent_transitions_single_winner() {
    let dir = tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("f06.db").display()
        ))
        .await
        .unwrap();
    m31a::persistence::sqlite::schema::run_migrations(&pool)
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Executing", "/tmp/ws").await;

    let barrier = Arc::new(Barrier::new(4));
    let targets = [
        ExecutionRunState::Paused,
        ExecutionRunState::Cancelled,
        ExecutionRunState::Completed,
        ExecutionRunState::Failed,
    ];
    let mut handles = Vec::new();
    for target in targets {
        let p = pool.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            let repo = SqliteExecutionRunRepository::new(p);
            repo.update_status(run, target).await.is_ok()
        }));
    }
    let mut wins = 0;
    for h in handles {
        if h.await.unwrap() {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one concurrent transition must win");
    let final_state = run_status(&pool, run).await;
    assert!(
        ["Paused", "Cancelled", "Completed", "Failed"].contains(&final_state.as_str()),
        "final state must be a valid Executing successor, got: {final_state}"
    );
}

#[tokio::test]
async fn f06_terminal_run_rejects_all_ops() {
    let dir = tempdir().unwrap();
    let runtime = setup_runtime_with_caller(&dir, CompleteModelCaller).await;
    let run = ExecutionRunId::new();
    seed_execution_run(
        runtime.pool(),
        run,
        "Completed",
        &dir.path().to_string_lossy(),
    )
    .await;

    assert!(runtime.pause_execution(run, "x").await.is_err());
    assert!(runtime.resume_mission(run).await.is_err());
    assert!(runtime.cancel_execution(run, "x").await.is_err());
    assert_eq!(run_status(runtime.pool(), run).await, "Completed");

    // Direct repository leaps are rejected too.
    let repo = SqliteExecutionRunRepository::new(runtime.pool().clone());
    assert!(
        repo.update_status(run, ExecutionRunState::Executing)
            .await
            .is_err()
    );
    assert!(
        repo.update_status(run, ExecutionRunState::Paused)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn f06_created_to_paused_leap_rejected() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f06.db"))
        .await
        .unwrap();
    let run = ExecutionRunId::new();
    seed_execution_run(&pool, run, "Created", "/tmp/ws").await;
    let repo = SqliteExecutionRunRepository::new(pool);
    assert!(
        repo.update_status(run, ExecutionRunState::Paused)
            .await
            .is_err()
    );
}

// ============================================================================
// F-07: approval waiter lifecycle (deterministic, no sleeps)
// ============================================================================

async fn wait_for_waiter(coord: &ApprovalCoordinator, id: ApprovalRequestId) {
    let start = std::time::Instant::now();
    while !coord.has_active_waiter(&id).await {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "waiter never registered"
        );
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn f07_immediate_approval_and_denial_received() {
    for (action, allowed) in [
        (ApprovalAction::AllowOnce, true),
        (
            ApprovalAction::Deny {
                reason: "no".to_string(),
            },
            false,
        ),
    ] {
        let (coord, holder) = immediate_coordinator(None, action);
        set_channel_holder(&holder, &coord).await;
        let req = make_approval_request(MissionId::new(), None);
        let out = coord
            .request_approval(req, AutonomyMode::Autonomous, Duration::from_secs(5))
            .await
            .expect("immediate resolution must be received");
        assert_eq!(out.is_allowed(), allowed);
        assert_eq!(coord.active_waiters_count().await, 0);
        assert_eq!(coord.task_mapping_count().await, 0);
        assert!(coord.pending_request_ids().await.is_empty());
    }
}

#[tokio::test]
async fn f07_notifier_failure_cancels_durable_state() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f07.db"))
        .await
        .unwrap();
    let coord = ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(Arc::new(FailingApprovalChannel) as Arc<dyn ApprovalChannel>),
    );
    let mission = MissionId::new();
    let task = TaskId::new();
    seed_approval_scope(&pool, mission, task).await;
    let req = make_approval_request(mission, Some(task));
    let req_id = req.id;

    let err = coord
        .request_approval(req, AutonomyMode::Autonomous, Duration::from_secs(5))
        .await
        .expect_err("notifier failure must fail the request");
    assert!(
        matches!(err, ApprovalError::ChannelClosed(_)),
        "got: {err:?}"
    );

    // Durable row is cancelled (recoverable terminal), never stranded pending.
    let state: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "cancelled");
    assert_eq!(coord.active_waiters_count().await, 0);
    assert_eq!(coord.task_mapping_count().await, 0);
    // Late resolution is rejected, not revived.
    let late = coord
        .resolve_request(req_id, ApprovalAction::AllowOnce, "late")
        .await
        .expect_err("late resolve of cancelled request must fail");
    assert!(matches!(late, ApprovalError::Cancelled(_)), "got: {late:?}");
}

#[tokio::test]
async fn f07_timeout_cleans_all_registrations() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f07.db"))
        .await
        .unwrap();
    let coord = ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(Arc::new(MockApprovalChannel::default()) as Arc<dyn ApprovalChannel>),
    );
    let mission = MissionId::new();
    let task = TaskId::new();
    seed_approval_scope(&pool, mission, task).await;
    let req = make_approval_request(mission, Some(task));
    let req_id = req.id;

    let err = coord
        .request_approval(req, AutonomyMode::Autonomous, Duration::from_millis(80))
        .await
        .expect_err("unanswered request must time out");
    assert!(matches!(err, ApprovalError::Timeout(_)), "got: {err:?}");

    let state: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "expired");
    assert_eq!(coord.active_waiters_count().await, 0);
    assert_eq!(coord.task_mapping_count().await, 0);
    assert!(!coord.has_active_waiter(&req_id).await);
    // Duplicate reply after expiry is rejected.
    let dup = coord
        .resolve_request(req_id, ApprovalAction::AllowOnce, "duplicate")
        .await
        .expect_err("post-expiry resolve must fail");
    assert!(matches!(dup, ApprovalError::Timeout(_)), "got: {dup:?}");
}

#[tokio::test]
async fn f07_concurrent_resolvers_single_winner() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f07.db"))
        .await
        .unwrap();
    let coord = Arc::new(ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(Arc::new(MockApprovalChannel::default()) as Arc<dyn ApprovalChannel>),
    ));
    let mission = MissionId::new();
    let task = TaskId::new();
    seed_approval_scope(&pool, mission, task).await;
    let req = make_approval_request(mission, Some(task));
    let req_id = req.id;

    let c = coord.clone();
    let requester = tokio::spawn(async move {
        c.request_approval(req, AutonomyMode::Autonomous, Duration::from_secs(10))
            .await
    });
    wait_for_waiter(&coord, req_id).await;

    let barrier = Arc::new(Barrier::new(3));
    let mut resolvers = Vec::new();
    for action in [
        ApprovalAction::AllowOnce,
        ApprovalAction::Deny {
            reason: "r1".to_string(),
        },
        ApprovalAction::Deny {
            reason: "r2".to_string(),
        },
    ] {
        let cc = coord.clone();
        let b = barrier.clone();
        resolvers.push(tokio::spawn(async move {
            b.wait().await;
            cc.resolve_request(req_id, action, "racer").await.is_ok()
        }));
    }
    let mut wins = 0;
    for r in resolvers {
        if r.await.unwrap() {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one concurrent resolver must win");
    let winner_action = requester.await.unwrap().expect("winner must deliver");
    let durable: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        durable,
        if winner_action.is_allowed() {
            "approved"
        } else {
            "denied"
        },
        "durable state must match the delivered outcome"
    );
    assert_eq!(coord.active_waiters_count().await, 0);
    assert_eq!(coord.task_mapping_count().await, 0);
}

#[tokio::test]
async fn f07_cancel_late_duplicate_and_no_leak() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f07.db"))
        .await
        .unwrap();
    let coord = Arc::new(ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(Arc::new(MockApprovalChannel::default()) as Arc<dyn ApprovalChannel>),
    ));
    let mission = MissionId::new();
    let task = TaskId::new();
    seed_approval_scope(&pool, mission, task).await;
    let req = make_approval_request(mission, Some(task));
    let req_id = req.id;

    let c = coord.clone();
    let requester = tokio::spawn(async move {
        c.request_approval(req, AutonomyMode::Autonomous, Duration::from_secs(10))
            .await
    });
    wait_for_waiter(&coord, req_id).await;
    coord.cancel_task_approvals(task).await;

    let out = requester.await.unwrap().expect("cancel must deliver");
    assert!(
        matches!(out, ApprovalAction::DenyAndCancelTask { .. }),
        "cancelled worker must receive DenyAndCancelTask, got: {out:?}"
    );
    let durable: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(durable, "cancelled");
    assert_eq!(coord.active_waiters_count().await, 0);
    assert_eq!(coord.task_mapping_count().await, 0);

    // Late reply cannot revive the cancelled request.
    let late = coord
        .resolve_request(req_id, ApprovalAction::AllowOnce, "late")
        .await
        .expect_err("late resolve must fail");
    assert!(matches!(late, ApprovalError::Cancelled(_)), "got: {late:?}");

    // Repeated approval cycles leave no residue (in-memory coordinator:
    // the leak vectors under test are the waiter and task-mapping tables).
    let (sync_coord, holder) = immediate_coordinator(None, ApprovalAction::AllowOnce);
    set_channel_holder(&holder, &sync_coord).await;
    for _ in 0..30 {
        let r = make_approval_request(MissionId::new(), Some(TaskId::new()));
        let o = sync_coord
            .request_approval(r, AutonomyMode::Autonomous, Duration::from_secs(5))
            .await
            .expect("cycle must resolve");
        assert!(o.is_allowed());
    }
    assert_eq!(sync_coord.active_waiters_count().await, 0);
    assert_eq!(sync_coord.task_mapping_count().await, 0);
    assert!(sync_coord.pending_request_ids().await.is_empty());
}

// ============================================================================
// F-08: baseline capture — symlink containment + completeness honesty
// ============================================================================

#[cfg(unix)]
fn symlink(path: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, path).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn f08_file_symlink_outside_excluded() {
    let outer = tempdir().unwrap();
    let secret = outer.path().join("outside_secret.txt");
    std::fs::write(&secret, b"outside").unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("real.txt"), b"inside").unwrap();
    symlink(&ws.join("evil.txt"), &secret);

    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(report.complete);
    assert!(report.baseline.file_hashes.contains_key("real.txt"));
    assert!(
        !report.baseline.file_hashes.contains_key("evil.txt"),
        "outside symlink must never contribute content"
    );
    assert!(
        report.excluded.iter().any(|e| e.contains("evil.txt")),
        "exclusion must be recorded, got: {:?}",
        report.excluded
    );
}

#[cfg(unix)]
#[tokio::test]
async fn f08_dir_symlink_outside_not_traversed() {
    let outer = tempdir().unwrap();
    let outside = outer.path().join("outside_dir");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("inner.txt"), b"outside").unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    symlink(&ws.join("evil_dir"), &outside);

    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(report.complete);
    assert!(
        !report
            .baseline
            .file_hashes
            .keys()
            .any(|k| k.contains("inner.txt")),
        "outside directory must never be traversed"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn f08_symlinks_into_protected_state_excluded() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(ws.join(".git")).unwrap();
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::fs::write(ws.join(".git").join("config"), b"git-config").unwrap();
    std::fs::write(ws.join(".m31a").join("state.db"), b"runtime-state").unwrap();
    std::fs::write(ws.join("ok.txt"), b"ok").unwrap();
    symlink(&ws.join("link_git"), &ws.join(".git").join("config"));
    symlink(&ws.join("link_m31a"), &ws.join(".m31a").join("state.db"));

    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(report.complete);
    assert!(!report.baseline.file_hashes.contains_key("link_git"));
    assert!(!report.baseline.file_hashes.contains_key("link_m31a"));
    assert!(report.baseline.file_hashes.contains_key("ok.txt"));
    // Protected content itself never hashed either.
    assert!(
        !report
            .baseline
            .file_hashes
            .keys()
            .any(|k| k.contains(".git") || k.contains(".m31a")),
        "protected content must never enter the baseline"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn f08_symlink_loops_terminate() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(ws.join("a").join("b")).unwrap();
    std::fs::write(ws.join("a").join("f.txt"), b"f").unwrap();
    // Self loop, parent loop, and a two-directory cycle.
    symlink(&ws.join("loop_self"), &ws);
    symlink(&ws.join("a").join("loop_parent"), &ws);
    symlink(&ws.join("a").join("b").join("to_a"), &ws.join("a"));
    symlink(&ws.join("to_b"), &ws.join("a").join("b"));

    let started = std::time::Instant::now();
    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "traversal must terminate despite loops"
    );
    assert!(report.complete);
    assert!(report.baseline.file_hashes.contains_key("a/f.txt"));
}

#[cfg(unix)]
#[tokio::test]
async fn f08_unreadable_entries_make_capture_incomplete() {
    let check_root = id_is_privileged();
    if check_root {
        eprintln!("SKIP: running with privileges; unreadable-file simulation unavailable");
        return;
    }
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("ok.txt"), b"ok").unwrap();
    let locked = ws.join("locked.txt");
    std::fs::write(&locked, b"nope").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(
        std::fs::read(&locked).is_err(),
        "precondition: file must be unreadable"
    );

    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(
        !report.complete,
        "unreadable file must mark capture incomplete"
    );
    assert!(
        report
            .skipped_errors
            .iter()
            .any(|e| e.contains("locked.txt")),
        "failure must name the entry, got: {:?}",
        report.skipped_errors
    );
    assert!(report.baseline.file_hashes.contains_key("ok.txt"));
    assert!(
        RepositoryBaseline::capture(&ws, MissionId::new(), None, None).is_err(),
        "strict capture must refuse an incomplete baseline"
    );
    // Restore permissions so TempDir cleanup succeeds.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
fn id_is_privileged() -> bool {
    // euid 0 can read through 0o000; detect and let callers skip.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn id_is_privileged() -> bool {
    false
}

#[tokio::test]
async fn f08_large_nested_tree_complete() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    let mut expected = 0;
    for d in 0..200 {
        let sub = ws.join(format!("d{d:03}"));
        std::fs::create_dir_all(&sub).unwrap();
        for f in 0..5 {
            std::fs::write(sub.join(format!("f{f}.txt")), format!("data {d} {f}")).unwrap();
            expected += 1;
        }
    }
    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(report.complete);
    assert_eq!(report.baseline.file_hashes.len(), expected);
}

#[cfg(unix)]
#[tokio::test]
async fn f08_internal_file_symlink_captured() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("real.txt"), b"shared content").unwrap();
    symlink(&ws.join("alias.txt"), &ws.join("real.txt"));

    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(report.complete);
    assert_eq!(
        report.baseline.file_hashes.get("alias.txt"),
        report.baseline.file_hashes.get("real.txt"),
        "internal file symlink must hash target content"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn f08_dangling_symlink_retarget_detected() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    symlink(
        &ws.join("dangling.txt"),
        &PathBuf::from("no/such/target.txt"),
    );

    let before =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(
        before.complete,
        "dangling link carries link-target evidence"
    );
    assert!(before.baseline.file_hashes.contains_key("dangling.txt"));

    // Retarget the link: drift must be detected without ever traversing.
    std::fs::remove_file(ws.join("dangling.txt")).unwrap();
    symlink(&ws.join("dangling.txt"), &PathBuf::from("other/target.txt"));
    let after = RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    let authorized = std::collections::BTreeSet::new();
    let drift = detect_drift(&before.baseline, &after.baseline.file_hashes, &authorized);
    assert!(
        drift.has_drift,
        "symlink retargeting must be detected as drift"
    );
}

#[tokio::test]
async fn f08_excessive_depth_incomplete() {
    let outer = tempdir().unwrap();
    let mut deep = outer.path().join("ws");
    std::fs::create_dir_all(&deep).unwrap();
    for i in 0..140 {
        deep = deep.join(format!("l{i}"));
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("deep.txt"), b"deep").unwrap();

    let report = RepositoryBaseline::capture_with_report(
        &outer.path().join("ws"),
        MissionId::new(),
        None,
        None,
    )
    .unwrap();
    assert!(
        !report.complete,
        "unbounded depth must be reported incomplete, not traversed forever"
    );
    assert!(
        RepositoryBaseline::capture(&outer.path().join("ws"), MissionId::new(), None, None)
            .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn f08_fifo_excluded_without_blocking() {
    let outer = tempdir().unwrap();
    let ws = outer.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("ok.txt"), b"ok").unwrap();
    let fifo = ws.join("pipe.fifo");
    let cpath = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
    let rc = unsafe { libc::mkfifo(cpath.as_ptr(), 0o644) };
    assert_eq!(rc, 0, "mkfifo must succeed");

    let started = std::time::Instant::now();
    let report =
        RepositoryBaseline::capture_with_report(&ws, MissionId::new(), None, None).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "fifo must not block capture"
    );
    assert!(report.complete);
    assert!(!report.baseline.file_hashes.contains_key("pipe.fifo"));
    assert!(report.baseline.file_hashes.contains_key("ok.txt"));
}

// ============================================================================
// F-13: per-execution approval configuration isolation
// ============================================================================

#[tokio::test]
async fn f13_concurrent_missions_isolated() {
    let coord = ApprovalCoordinator::new(None, None);
    let mission_a = MissionId::new();
    let mission_b = MissionId::new();
    coord.set_wait_for_approval_for(mission_a, true);
    coord.set_wait_for_approval_for(mission_b, false);

    // B never waits: immediate deny.
    let req_b = make_approval_request(mission_b, None);
    let out_b = coord
        .request_approval(req_b, AutonomyMode::Autonomous, Duration::from_secs(5))
        .await
        .expect("B must resolve");
    assert!(
        matches!(out_b, ApprovalAction::Deny { .. }),
        "mission B (no wait) must deny immediately, got: {out_b:?}"
    );

    // A waits: unanswered request times out instead of denying.
    let req_a = make_approval_request(mission_a, None);
    let err_a = coord
        .request_approval(req_a, AutonomyMode::Autonomous, Duration::from_millis(150))
        .await
        .expect_err("mission A (wait) must block until timeout");
    assert!(matches!(err_a, ApprovalError::Timeout(_)), "got: {err_a:?}");

    // Order reversed: A resolves via channel while B still denies instantly.
    let (coord2, holder) = immediate_coordinator(None, ApprovalAction::AllowOnce);
    set_channel_holder(&holder, &coord2).await;
    coord2.set_wait_for_approval_for(mission_a, true);
    coord2.set_wait_for_approval_for(mission_b, false);
    let out_a2 = coord2
        .request_approval(
            make_approval_request(mission_a, None),
            AutonomyMode::Autonomous,
            Duration::from_secs(5),
        )
        .await
        .expect("A must be allowed");
    assert!(out_a2.is_allowed());
    let out_b2 = coord2
        .request_approval(
            make_approval_request(mission_b, None),
            AutonomyMode::Autonomous,
            Duration::from_secs(5),
        )
        .await
        .expect("B must resolve");
    assert!(!out_b2.is_allowed());

    // Unrelated missions still see the untouched process default (true).
    assert!(coord2.effective_wait_for_approval(MissionId::new()));
}

#[tokio::test]
async fn f13_unattended_denies_despite_wait() {
    let coord = ApprovalCoordinator::new(None, None);
    let mission = MissionId::new();
    coord.set_wait_for_approval_for(mission, true);
    let out = coord
        .request_approval(
            make_approval_request(mission, None),
            AutonomyMode::Unattended,
            Duration::from_secs(5),
        )
        .await
        .expect("unattended must resolve");
    assert!(
        matches!(out, ApprovalAction::Deny { .. }),
        "unattended mode must deny even with wait=true, got: {out:?}"
    );
}

#[tokio::test]
async fn f13_per_request_override_wins() {
    let (coord, holder) = immediate_coordinator(None, ApprovalAction::AllowOnce);
    set_channel_holder(&holder, &coord).await;
    let mission = MissionId::new();

    // Execution says wait, request says do not: deny without blocking.
    coord.set_wait_for_approval_for(mission, true);
    let out = coord
        .request_approval_with_options(
            make_approval_request(mission, None),
            AutonomyMode::Autonomous,
            Duration::from_secs(5),
            Some(false),
        )
        .await
        .expect("override must resolve");
    assert!(!out.is_allowed());

    // Execution says do not wait, request says wait: channel resolves allow.
    coord.set_wait_for_approval_for(mission, false);
    let out2 = coord
        .request_approval_with_options(
            make_approval_request(mission, None),
            AutonomyMode::Autonomous,
            Duration::from_secs(5),
            Some(true),
        )
        .await
        .expect("override must resolve");
    assert!(out2.is_allowed());
}

// ============================================================================
// F-14: truthful workspace fingerprints (unchanged / changed / unknown)
// ============================================================================

#[tokio::test]
async fn f14_same_size_content_change_detected() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("code.rs"), b"fn main() { alpha(); }").unwrap();

    let runtime = AppRuntime::new(&ws).await.expect("runtime");
    let engine = runtime.create_agent_engine(m31a::ids::SessionId::new());
    let fp1 = engine.compute_workspace_fingerprint().await;
    assert_eq!(fp1.len(), 64);

    // Same byte length, different content: hash must still change.
    std::fs::write(ws.join("code.rs"), b"fn main() { betaa(); }").unwrap();
    let fp2 = engine.compute_workspace_fingerprint().await;
    assert_ne!(
        fp1, fp2,
        "content change without size change must be detected"
    );

    // Deletion changes it too.
    std::fs::remove_file(ws.join("code.rs")).unwrap();
    let fp3 = engine.compute_workspace_fingerprint().await;
    assert_ne!(fp2, fp3);
}

#[tokio::test]
async fn f14_unavailable_fingerprint_is_unknown() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("ok.txt"), b"ok").unwrap();
    #[cfg(unix)]
    {
        if id_is_privileged() {
            eprintln!("SKIP: privileged; unreadable simulation unavailable");
            return;
        }
        let locked = ws.join("locked.txt");
        std::fs::write(&locked, b"nope").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert!(std::fs::read(&locked).is_err());

        let runtime = AppRuntime::new(&ws).await.expect("runtime");
        let engine = runtime.create_agent_engine(m31a::ids::SessionId::new());
        let fp = engine.compute_workspace_fingerprint().await;
        assert!(
            fp.starts_with(UNKNOWN_FINGERPRINT_PREFIX),
            "failed capture must yield an unknown marker, got: {fp}"
        );
        assert_ne!(
            fp.len(),
            64,
            "unknown marker must never masquerade as content hash"
        );
        // Unknown states never compare equal: no false stagnation.
        let fp2 = engine.compute_workspace_fingerprint().await;
        assert!(fp2.starts_with(UNKNOWN_FINGERPRINT_PREFIX));
        assert_ne!(fp, fp2);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

#[test]
fn f14_tristate_and_no_false_stagnation() {
    assert_eq!(
        compare_workspace_fingerprints("abc", "abc"),
        FingerprintComparison::Unchanged
    );
    assert_eq!(
        compare_workspace_fingerprints("abc", "abd"),
        FingerprintComparison::Changed
    );
    assert_eq!(
        compare_workspace_fingerprints("unknown-1", "unknown-1"),
        FingerprintComparison::Unknown,
        "identical unknown markers are still unknown, never unchanged"
    );
    assert_eq!(
        compare_workspace_fingerprints("abc", "unknown-2"),
        FingerprintComparison::Unknown
    );

    // Stall detector: identical failures with unchanged content stagnate...
    let mut detector = StallDetector::new();
    let obs = |ws: &str| ExecutionObservation {
        tool_name: "run_tests".to_string(),
        input_fingerprint: "in1".to_string(),
        workspace_state_fingerprint: ws.to_string(),
        success: false,
        failure_signature: Some("boom".to_string()),
        timestamp: chrono::Utc::now(),
    };
    detector.record_observation(obs("fff"));
    detector.record_observation(obs("fff"));
    assert!(matches!(
        detector.evaluate_proposal("run_tests", "in1", "fff"),
        StallEvaluation::StagnationLoopDetected { .. }
    ));
    // ...but unknown evidence never fabricates stagnation.
    assert!(matches!(
        detector.evaluate_proposal("run_tests", "in1", "unknown-marker-1"),
        StallEvaluation::Progressing
    ));
}

// ============================================================================
// F-15: authorization ordering, bindings, revocation, staged rollback
// ============================================================================

fn f15_plan() -> CandidatePlan {
    let task = CandidateTask::new(
        "t1",
        "Do authorized work",
        AgentRole::implementer(),
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    CandidatePlan::new("plan-auth-1", "Authorized objective", vec![task])
}

fn f15_build_auth(
    runtime: &AppRuntime,
    session: &m31a::ids::SessionId,
    plan: &CandidatePlan,
) -> m31a::planning::review::ExecutionAuthorization {
    let plan_hash = m31a::planning::review::PlanRevision::compute_content_hash(plan);
    let task_hash = m31a::planning::review::TaskRevision::compute_tasks_hash(&plan.tasks);
    let policy_hash = runtime.policy().active_policy_hash().to_string();
    let ws = runtime.workspace_root().display().to_string();
    m31a::planning::review::ExecutionAuthorization::new(
        session.to_string(),
        plan.revision,
        1,
        "operator",
    )
    .with_content_hashes(plan_hash, task_hash)
    .with_execution_binding(
        policy_hash,
        ws,
        "implementer",
        "safe",
        Duration::from_secs(3600),
    )
}

async fn f15_save_auth(
    runtime: &AppRuntime,
    session: &m31a::ids::SessionId,
    plan: &CandidatePlan,
    auth: &m31a::planning::review::ExecutionAuthorization,
) {
    let coord = runtime.create_pre_execution_coordinator();
    // Production-faithful harness: the review flow persists plan/task
    // revisions alongside the authorization; boundary revalidation reads
    // all three, so all three are seeded here.
    let plan_rev = m31a::planning::review::PlanRevision::new(
        session.to_string(),
        plan.revision,
        plan.plan_id.clone(),
        plan.clone(),
        "operator",
        m31a::planning::review::RevisionAuthorType::User,
        None,
    );
    coord
        .lifecycle_repo()
        .save_plan_revision(&plan_rev)
        .await
        .expect("save plan revision");
    let task_rev = m31a::planning::review::TaskRevision::new(
        session.to_string(),
        1,
        plan.revision,
        plan.tasks.clone(),
        "operator",
        m31a::planning::review::RevisionAuthorType::User,
        None,
    );
    coord
        .lifecycle_repo()
        .save_task_revision(&task_rev)
        .await
        .expect("save task revision");
    coord
        .lifecycle_repo()
        .save_execution_authorization(auth)
        .await
        .expect("save auth");
    coord
        .lifecycle_repo()
        .save_lifecycle_state(&PersistedLifecycleState {
            session_id: session.to_string(),
            stage: m31a::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
            plan_revision: plan.revision,
            task_revision: 1,
            authorization_id: Some(auth.id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await
        .expect("save lifecycle");
}

async fn f15_valid_auth(
    runtime: &AppRuntime,
    session: &m31a::ids::SessionId,
    plan: &CandidatePlan,
) -> m31a::planning::review::ExecutionAuthorization {
    let auth = f15_build_auth(runtime, session, plan);
    f15_save_auth(runtime, session, plan, &auth).await;
    auth
}

async fn f15_counts(pool: &sqlx::SqlitePool) -> (i64, i64) {
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs")
        .fetch_one(pool)
        .await
        .unwrap();
    let graphs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_graphs")
        .fetch_one(pool)
        .await
        .unwrap();
    (runs, graphs)
}

#[tokio::test]
async fn f15_expired_authorization_creates_no_run() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    let plan = f15_plan();
    let mut auth = f15_build_auth(&runtime, &session.id, &plan);
    auth.expires_at = Some(chrono::Utc::now() - chrono::Duration::hours(1));
    f15_save_auth(&runtime, &session.id, &plan, &auth).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("expired authorization must be refused");
    assert!(
        err.to_string().to_lowercase().contains("expir"),
        "got: {err}"
    );
    assert_eq!(f15_counts(runtime.pool()).await, (0, 0));
}

#[tokio::test]
async fn f15_wrong_workspace_creates_no_run() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    let plan = f15_plan();
    let mut auth = f15_build_auth(&runtime, &session.id, &plan);
    auth.workspace_root = Some("/elsewhere/entirely".to_string());
    f15_save_auth(&runtime, &session.id, &plan, &auth).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("foreign workspace binding must be refused");
    assert!(
        err.to_string().to_lowercase().contains("workspace"),
        "got: {err}"
    );
    assert_eq!(f15_counts(runtime.pool()).await, (0, 0));
}

#[tokio::test]
async fn f15_stale_policy_hash_creates_no_run() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    let plan = f15_plan();
    let mut auth = f15_build_auth(&runtime, &session.id, &plan);
    auth.policy_hash = Some("stale-generation-hash".to_string());
    f15_save_auth(&runtime, &session.id, &plan, &auth).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("stale policy generation must be refused");
    assert!(
        err.to_string().to_lowercase().contains("policy"),
        "got: {err}"
    );
    assert_eq!(f15_counts(runtime.pool()).await, (0, 0));
}

#[tokio::test]
async fn f15_wrong_role_creates_no_run() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    let plan = f15_plan();
    let mut auth = f15_build_auth(&runtime, &session.id, &plan);
    auth.agent_role = Some("reviewer".to_string());
    f15_save_auth(&runtime, &session.id, &plan, &auth).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("wrong execution role must be refused");
    assert!(
        err.to_string().to_lowercase().contains("role"),
        "got: {err}"
    );
    assert_eq!(f15_counts(runtime.pool()).await, (0, 0));
}

#[tokio::test]
async fn f15_revocation_detected_at_boundary() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    let plan = f15_plan();
    let mut auth = f15_build_auth(&runtime, &session.id, &plan);
    auth.autonomy_mode = Some("Safe".to_string());
    f15_save_auth(&runtime, &session.id, &plan, &auth).await;

    let run = ExecutionRunId::new();
    runtime
        .session_repo()
        .set_active_run(session.id, run)
        .await
        .expect("associate");
    let repo = runtime.create_pre_execution_coordinator();
    let expectations = m31a::persistence::sqlite::repositories::lifecycle::ResumeAuthExpectations {
        policy_hash: runtime.policy().active_policy_hash().to_string(),
        workspace_root: runtime.workspace_root().display().to_string(),
        agent_role: "implementer".to_string(),
        autonomy_mode: "Safe".to_string(),
    };
    repo.lifecycle_repo()
        .revalidate_authorization_for_resume_with(run, Some(&expectations))
        .await
        .expect("pre-revocation boundary check must be Valid");

    repo.lifecycle_repo()
        .invalidate_authorizations(&session.id.to_string(), "operator revoked")
        .await
        .expect("revoke");
    let err = repo
        .lifecycle_repo()
        .revalidate_authorization_for_resume_with(run, Some(&expectations))
        .await
        .expect_err("post-revocation boundary check must fail");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("revok") || msg.contains("invalid") || msg.contains("authoriz"),
        "got: {err}"
    );
}

#[tokio::test]
async fn f15_staged_association_rolled_back() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("runtime");
    let runtime = Arc::new(runtime);
    let session = runtime.create_session().await.expect("session");
    // Plan whose DAG references a nonexistent prerequisite: authorization
    // validation passes (revisions/hashes bind), but graph materialization
    // (step 6) fails AFTER session association — exercising staged rollback.
    let mut plan = f15_plan();
    plan.tasks[0] =
        plan.tasks[0]
            .clone()
            .with_depends_on(vec![m31a::kernel::plan::CandidateTaskKey::new(
                "ghost-task",
            )]);
    let auth = f15_valid_auth(&runtime, &session.id, &plan).await;

    let err = runtime
        .start_authorized_execution(session.id, &plan, auth)
        .await
        .expect_err("invalid DAG must fail materialization");
    assert!(
        !err.to_string().contains("StartExecution"),
        "failure must come from materialization, got: {err}"
    );

    // Staged association is rolled back; the undispatched run row remains as
    // truthful staged state (Created, never Executing).
    let session_after = runtime.get_session(session.id).await.unwrap().unwrap();
    assert_eq!(
        session_after.active_run_id, None,
        "undispatched association must be rolled back"
    );
    let (runs, _) = f15_counts(runtime.pool()).await;
    assert_eq!(runs, 1, "created run row is preserved truthfully");
    let status: String = sqlx::query_scalar("SELECT status FROM execution_runs LIMIT 1")
        .fetch_one(runtime.pool())
        .await
        .unwrap();
    assert_eq!(
        status, "Created",
        "nothing dispatched: run never left Created"
    );
}

// ============================================================================
// F-10: mandatory output integrity evidence on every recovery path
// ============================================================================

async fn f10_seed_task(pool: &sqlx::SqlitePool, mission: MissionId, task: TaskId, status: &str) {
    let now = now_str();
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind("f10 task")
    .bind(status)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn f10_insert_artifact_row(
    pool: &sqlx::SqlitePool,
    mission: MissionId,
    task: TaskId,
    logical_path: &str,
    content_hash: &str,
) {
    let now = now_str();
    sqlx::query(
        r#"
        INSERT INTO artifacts (
            id, name, logical_path, content_hash, size_bytes, extension,
            mission_id, task_id, created_at
        ) VALUES (?, 'output', ?, ?, 10, 'txt', ?, ?, ?)
        "#,
    )
    .bind(ArtifactId::new().as_bytes().as_slice())
    .bind(logical_path)
    .bind(content_hash)
    .bind(mission.as_bytes().as_slice())
    .bind(task.as_bytes().as_slice())
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

fn f10_manifest(mission: MissionId, states: BTreeMap<TaskId, TaskState>) -> CheckpointManifest {
    CheckpointManifest::new(
        CheckpointId::new(),
        mission,
        1,
        "Observe",
        1,
        "snap",
        states,
        BTreeMap::new(),
        "policy",
        Vec::new(),
        Vec::new(),
        "f10",
    )
}

#[tokio::test]
async fn f10_same_size_tamper_invalidated_via_production_hydration() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f10.db"))
        .await
        .unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("art")));

    let mission = MissionId::new();
    let task = TaskId::new();
    seed_legacy_mission(&pool, mission, "in_progress").await;
    f10_seed_task(&pool, mission, task, "succeeded").await;

    // Real output + saved baseline (production evidence, no manual mapping).
    std::fs::write(ws.join("out.txt"), b"task 1 output data\n").unwrap();
    let baseline = RepositoryBaseline::capture(&ws, mission, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();
    f10_insert_artifact_row(&pool, mission, task, "out.txt", "hash-irrelevant").await;

    let mut states = BTreeMap::new();
    states.insert(task, TaskState::Succeeded);
    let manifest = f10_manifest(mission, states);

    // Same byte length, different content: hash validation must catch it.
    std::fs::write(ws.join("out.txt"), b"TASK 1 OUTPUT DATA\n").unwrap();
    assert_eq!(
        std::fs::read(ws.join("out.txt")).unwrap().len(),
        "task 1 output data\n".len()
    );

    let engine = SafeResumeEngine::new(pool.clone(), store, &ws);
    let mut hydrated = engine;
    hydrated.hydrate_task_output_files(mission).await.unwrap();
    let report = hydrated.resume_mission(mission, &manifest).await.unwrap();
    assert!(
        report.invalidated_tasks.contains(&task),
        "same-size tamper must invalidate"
    );
    assert!(report.preserved_tasks.is_empty());
}

#[tokio::test]
async fn f10_absent_mandatory_evidence_invalidated() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f10.db"))
        .await
        .unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("art")));

    let mission = MissionId::new();
    let task = TaskId::new();
    seed_legacy_mission(&pool, mission, "in_progress").await;
    // Succeeded task with NO output mappings, NO verification artifacts.
    f10_seed_task(&pool, mission, task, "succeeded").await;

    let mut states = BTreeMap::new();
    states.insert(task, TaskState::Succeeded);
    let engine = SafeResumeEngine::new(pool, store, &ws);
    let report = engine
        .resume_mission(mission, &f10_manifest(mission, states))
        .await
        .unwrap();
    assert!(
        report.invalidated_tasks.contains(&task),
        "success without any integrity evidence must not be preserved"
    );
}

#[tokio::test]
async fn f10_corrupt_provenance_fails_safe() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f10.db"))
        .await
        .unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("art")));

    let mission = MissionId::new();
    let task = TaskId::new();
    seed_legacy_mission(&pool, mission, "in_progress").await;
    f10_seed_task(&pool, mission, task, "succeeded").await;
    // Corrupt provenance JSON is the ONLY mapping source; nothing else.
    sqlx::query(
        r#"
        INSERT INTO change_provenance (
            id, proposal_id, task_id, mission_id, requirement_keys, affected_files,
            affected_symbols, pre_mutation_hash, post_mutation_hash, diff_summary,
            verification_passed, created_at
        ) VALUES (?, ?, ?, ?, '[]', ?, '[]', 'pre', 'post', 'summary', 1, ?)
        "#,
    )
    .bind(ToolCallId::new().as_bytes().as_slice())
    .bind(ToolCallId::new().as_bytes().as_slice())
    .bind(task.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind("not-json{{{")
    .bind(now_str())
    .execute(&pool)
    .await
    .unwrap();

    let mut states = BTreeMap::new();
    states.insert(task, TaskState::Succeeded);
    let engine = SafeResumeEngine::new(pool, store, &ws);
    let report = engine
        .resume_mission(mission, &f10_manifest(mission, states))
        .await
        .unwrap();
    assert!(
        report.invalidated_tasks.contains(&task),
        "corrupt provenance must fail safe to invalidation"
    );
}

#[tokio::test]
async fn f10_dependent_of_invalidated_task_invalidated() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("f10.db"))
        .await
        .unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("art")));

    let mission = MissionId::new();
    let task_a = TaskId::new();
    let task_b = TaskId::new();
    seed_legacy_mission(&pool, mission, "in_progress").await;
    f10_seed_task(&pool, mission, task_a, "succeeded").await;
    f10_seed_task(&pool, mission, task_b, "succeeded").await;
    std::fs::write(ws.join("a.txt"), b"output A").unwrap();
    std::fs::write(ws.join("b.txt"), b"output B").unwrap();
    let baseline = RepositoryBaseline::capture(&ws, mission, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();
    f10_insert_artifact_row(&pool, mission, task_a, "a.txt", "h").await;
    f10_insert_artifact_row(&pool, mission, task_b, "b.txt", "h").await;

    // B depends on A.
    let now = now_str();
    let graph = m31a::ids::TaskGraphId::new();
    sqlx::query(
        "INSERT INTO task_graphs (id, mission_id, revision, plan_id, status, created_at, updated_at) VALUES (?, ?, 1, 'p', 'active', ?, ?)",
    )
    .bind(graph.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_dependencies (task_graph_id, prerequisite_task_id, dependent_task_id) VALUES (?, ?, ?)",
    )
    .bind(graph.as_bytes().as_slice())
    .bind(task_a.as_bytes().as_slice())
    .bind(task_b.as_bytes().as_slice())
    .execute(&pool)
    .await
    .unwrap();

    // Tamper A's output only; B's own evidence is intact.
    std::fs::write(ws.join("a.txt"), b"output A!").unwrap();

    let mut states = BTreeMap::new();
    states.insert(task_a, TaskState::Succeeded);
    states.insert(task_b, TaskState::Succeeded);
    let engine = SafeResumeEngine::new(pool, store, &ws);
    let report = engine
        .resume_mission(mission, &f10_manifest(mission, states))
        .await
        .unwrap();
    assert!(report.invalidated_tasks.contains(&task_a));
    assert!(
        report.invalidated_tasks.contains(&task_b),
        "dependent of invalidated task must be invalidated despite intact own evidence"
    );
    assert!(report.preserved_tasks.is_empty());
}

#[tokio::test]
async fn f10_runtime_resume_tamper_invalidates() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let runtime = setup_runtime_with_caller(&dir, DecomposeAwareCaller).await;
    let pool = runtime.pool().clone();
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    let run = ExecutionRunId::new();
    let task = TaskId::new();
    seed_execution_run(&pool, run, "Paused", &ws.to_string_lossy()).await;
    // Promote to execution_runs-backed mission row for baseline FK comfort.
    seed_legacy_mission(&pool, run, "paused").await;
    f10_seed_task(&pool, run, task, "succeeded").await;

    std::fs::write(ws.join("out.txt"), b"stable output\n").unwrap();
    let baseline = RepositoryBaseline::capture(ws, run, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();

    // Verification evidence in the store + DB rows.
    let art = ArtifactId::new();
    let art_data = b"verification log".to_vec();
    let art_hash = sha256_hex(&art_data);
    let check = CheckId::new();
    let now = now_str();
    sqlx::query(
        r#"
        INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, evidence_artifact_id, summary, snapshot_hash, created_at)
        VALUES (?, ?, ?, 3, 'passed', 'cargo test', '{}', ?, 'ok', 'snap', ?)
        "#,
    )
    .bind(check.as_bytes().as_slice())
    .bind(run.as_bytes().as_slice())
    .bind(task.as_bytes().as_slice())
    .bind(art.as_bytes().as_slice())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    f10_insert_artifact_row(&pool, run, task, "out.txt", &art_hash).await;

    let mut states = BTreeMap::new();
    states.insert(task, TaskState::Succeeded);
    let manifest = CheckpointManifest::new(
        CheckpointId::new(),
        run,
        1,
        "Observe",
        1,
        "snap",
        states,
        BTreeMap::new(),
        "policy",
        vec![check],
        vec![(art, art_hash, art_data.len() as u64)],
        "runtime resume",
    );
    let staging = dir.path().join("staging");
    // The runtime validates checkpoint artifacts against ITS OWN store, so
    // the checkpoint must be created there (production wiring, not a fork).
    let rt_store: Arc<dyn ArtifactStore> = runtime.artifact_store().clone();
    let manager = CheckpointManager::new(pool.clone(), rt_store, staging);
    manager
        .create_checkpoint(
            &manifest,
            vec![StagedArtifactInput::new(
                art,
                art_data,
                "bin",
                "verification_log",
            )],
        )
        .await
        .expect("create checkpoint");

    // Crash-like drift after the checkpoint.
    std::fs::write(ws.join("out.txt"), b"TAMPERED output!\n").unwrap();

    runtime
        .resume_mission(run)
        .await
        .expect("resume with drift must still dispatch (task invalidated, work continues)");
    let task_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        task_status, "pending",
        "drifted output must invalidate the task on the production resume path"
    );
    let envs = drain_events(&mut rx, Duration::from_secs(1)).await;
    assert_eq!(count_events(&envs, run, "resumed"), 1);
    assert_eq!(count_events(&envs, run, "failed"), 0);
}

// ============================================================================
// F-16: prior-pass preservation guard — unicode-safe redaction
// ============================================================================

#[test]
fn f16_unicode_redaction_safe() {
    let redactor = m31a::telemetry::redactor::SecretRedactor::new();
    redactor.register_secret("🔑sëcret✓token");
    let out = redactor.redact_string("prefix-🔑sëcret✓token-suffix 😀");
    assert_eq!(out, "prefix-[REDACTED:EXACT_SECRET]-suffix 😀");
    // Pure-unicode input without secrets passes through byte-intact.
    let innocent = "日本語テスト🎉ña";
    assert_eq!(redactor.redact_string(innocent), innocent);
}

// ============================================================================
// F-17: exact file-content fidelity
// ============================================================================

fn f17_ctx(dir: &tempfile::TempDir) -> ToolExecutionContext {
    let provider = Arc::new(LocalFileSystemProvider::new(dir.path()).unwrap());
    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_filesystem(provider);
    ToolExecutionContext::new(registry, dir.path().to_path_buf(), CancellationToken::new())
}

async fn f17_write_read(
    ctx: &ToolExecutionContext,
    dir: &tempfile::TempDir,
    path: &str,
    content: &str,
) -> Vec<u8> {
    WriteFileTool
        .execute(
            ctx,
            WriteFileInput {
                path: path.to_string(),
                content: content.to_string(),
            },
        )
        .await
        .expect("write must succeed");
    std::fs::read(dir.path().join(path)).unwrap()
}

#[tokio::test]
async fn f17_write_preserves_exact_bytes() {
    let dir = tempdir().unwrap();
    let ctx = f17_ctx(&dir);
    let cases = vec![
        ("spaces.txt", "  leading and trailing  \n"),
        ("newlines.txt", "first line\n\n\n"),
        ("tabs.txt", "\tindented\n\t\tdouble\n"),
        ("empty.txt", ""),
        ("unicode.txt", "héllo wörld 🎉\n"),
        (
            "markdown.md",
            "# Title\n\nHere is a block:\n```rust\nlet x = 1;\n```\nDone.\n",
        ),
        ("single_line_rs.rs", "fn f() {\\nlet x = 1;\\n}"),
        ("single_line_toml.toml", "a = \"x\";\\nb = \"y\""),
        ("double_escaped.rs", "let s = \"a\\\\nb\";\n"),
        ("rust_literal.rs", "println!(\"line1\\nline2\");\n"),
    ];
    for (path, content) in cases {
        let bytes = f17_write_read(&ctx, &dir, path, content).await;
        assert_eq!(
            bytes,
            content.as_bytes(),
            "file '{path}' must round-trip byte-exact"
        );
    }
}

#[tokio::test]
async fn f17_wrapper_envelopes_recognized_malformed_preserved() {
    let dir = tempdir().unwrap();
    let ctx = f17_ctx(&dir);

    // Genuine JSON content envelope is extracted.
    let bytes = f17_write_read(&ctx, &dir, "j.txt", "{\n  \"content\": \"inner\\n\"\n}").await;
    assert_eq!(bytes, b"inner\n");

    // Genuine full code-fence envelope is extracted.
    let bytes = f17_write_read(&ctx, &dir, "f.rs", "```rust\nlet x = 1;\n```").await;
    assert_eq!(bytes, b"let x = 1;");

    // Malformed wrapper (unclosed fence) is preserved byte-exact.
    let malformed = "```rust\nlet x = 1;\n";
    let bytes = f17_write_read(&ctx, &dir, "m.rs", malformed).await;
    assert_eq!(bytes, malformed.as_bytes());
}

#[tokio::test]
async fn f17_edit_preserves_literal_sequences() {
    let dir = tempdir().unwrap();
    let ctx = f17_ctx(&dir);
    std::fs::write(dir.path().join("e.rs"), "let a = 1;\nlet b = 2;\n").unwrap();
    let out = EditFileTool
        .execute(
            &ctx,
            EditFileInput {
                path: "e.rs".to_string(),
                old_content: "let a = 1;".to_string(),
                new_content: "let a = \"x\\ny\";".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect("edit must succeed");
    assert!(out.success);
    let content = std::fs::read_to_string(dir.path().join("e.rs")).unwrap();
    assert_eq!(content, "let a = \"x\\ny\";\nlet b = 2;\n");
}
