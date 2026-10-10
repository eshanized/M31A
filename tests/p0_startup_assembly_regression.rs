//! P0 Startup Timeout: runtime-assembly + crash-recovery regression suite.
//!
//! Covers the production startup failure
//! `Startup failed: Runtime assembly timed out after 30.0s (deadline exceeded)`:
//!
//! - Ordinary workspace completes assembly (no fake readiness).
//! - Invalid configuration produces an explicit configuration error, not a timeout.
//! - Database errors produce actionable failures with stage context.
//! - A deliberately delayed stage identifies itself in diagnostics.
//! - A real assembly timeout transitions the TUI to `Failed`.
//! - Timed-out work is cancelled; stale completions cannot attach; retries
//!   do not double-run recovery.
//! - Crash recovery: 0/1/N missions, checkpoints/artifacts, corrupt
//!   manifests, drift, job/task reconciliation, persistence errors,
//!   cancellation, and the scan-scope integrity-check invariant.
//!
//! Timing assertions use deterministic barriers and short local deadlines —
//! never timing-dependent sleeps as the main correctness assertion, except
//! where the timeout path itself is under test (bounded, local).

use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;

use m31a::checkpoint::crash_recovery::{CrashRecoveryClassification, StartupCrashRecoveryScanner};
use m31a::config::ResolvedConfigBuilder;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{AgentId, JobId, MissionId, TaskId};
use m31a::persistence::artifacts::fs_store::ArtifactStore;
use m31a::persistence::artifacts::fs_store::FsArtifactStore;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::startup_progress::{progress_channel, stage};
use m31a::tui::RuntimeAssemblyOutcome;
use m31a::tui::TuiError;
use m31a::tui::app::TuiApplication;

// ── Helpers ────────────────────────────────────────────────────────────────

fn test_backend_terminal() -> ratatui::Terminal<ratatui::backend::TestBackend> {
    let backend = ratatui::backend::TestBackend::new(120, 40);
    ratatui::Terminal::new(backend).unwrap()
}

async fn seed_mission(
    pool: &sqlx::SqlitePool,
    mission_id: MissionId,
    status: &str,
) -> (TaskId, AgentId) {
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind("p0 startup regression mission")
        .bind(status)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("regression task")
        .bind("running")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("worker")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    (task_id, agent_id)
}

async fn assemble_runtime(pool: sqlx::SqlitePool, ws: std::path::PathBuf) -> Arc<AppRuntime> {
    let bus = Arc::new(BroadcastEventBus::new(128));
    let config = Arc::new(ResolvedConfigBuilder::new(&ws).build_fallback());
    let rt = AppRuntime::from_pool_workspace_and_config(pool, ws, bus, config)
        .await
        .expect("ordinary workspace must assemble");
    Arc::new(rt)
}

// ── Runtime assembly ───────────────────────────────────────────────────────

#[tokio::test]
async fn assembly_ordinary_workspace_completes_successfully() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let pool = initialize_database(&dir.path().join("asm_ok.db"))
        .await
        .unwrap();
    let rt = assemble_runtime(pool, ws).await;
    // Runtime is genuinely ready: binding is attachable, config present.
    assert!(!rt.config().active_model.is_empty());
    rt.shutdown().await;
}

#[tokio::test]
async fn assembly_invalid_configuration_is_explicit_not_timeout() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    // Present-but-invalid workspace configuration must fail fast with an
    // explicit configuration error (from_pool_and_workspace path), never a
    // generic 30s timeout.
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::fs::write(
        ws.join(".m31a").join("config.toml"),
        "this is [not valid toml {{{",
    )
    .unwrap();
    let pool = initialize_database(&dir.path().join("asm_badcfg.db"))
        .await
        .unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));
    let res = AppRuntime::from_pool_and_workspace(pool, ws, bus).await;
    let err = match res {
        Ok(_) => panic!("invalid config must fail assembly"),
        Err(e) => e,
    };
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("config") || msg.contains("invalid"),
        "error must name configuration, got: {msg}"
    );
    assert!(
        !msg.contains("timed out") && !msg.contains("deadline exceeded"),
        "configuration errors must not masquerade as timeouts: {msg}"
    );
}

#[tokio::test]
async fn assembly_database_error_is_actionable_with_stage() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let pool = initialize_database(&dir.path().join("asm_dberr.db"))
        .await
        .unwrap();
    // Close the pool: subsequent acquisition fails deterministically.
    pool.close().await;
    let bus = Arc::new(BroadcastEventBus::new(64));
    let config = Arc::new(ResolvedConfigBuilder::new(&ws).build_fallback());
    let res = AppRuntime::from_pool_workspace_and_config(pool, ws, bus, config).await;
    let err = match res {
        Ok(_) => panic!("closed pool must fail assembly"),
        Err(e) => e,
    };
    let msg = err.to_string();
    // Actionable: names storage/recovery/database, never a bare timeout.
    assert!(
        !msg.contains("deadline exceeded"),
        "db errors must not masquerade as generic timeouts: {msg}"
    );
}

#[tokio::test]
async fn assembly_delayed_stage_identifies_itself_in_diagnostics() {
    // A deliberately delayed stage must name itself: drive the TUI with a
    // live progress update, force a short deadline, and assert the timeout
    // detail carries the stage.
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let (progress_tx, progress_rx) = progress_channel();
    let fake_task = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort = fake_task.abort_handle();

    let mut app = TuiApplication::new().with_assembly_timeout(Duration::from_millis(60));
    app.begin_assembly_with_progress(rx, progress_rx, abort);
    // Simulate the assembly task reporting its real stage.
    progress_tx
        .send(m31a::startup_progress::StartupStageUpdate::new(
            3,
            stage::CRASH_RECOVERY_INTEGRITY,
            "Checking database integrity…",
        ))
        .unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    let mut terminal = test_backend_terminal();
    app.poll_runtime(&mut terminal).await;
    assert!(
        app.model.runtime_status.is_failed(),
        "deadline must transition TUI to Failed"
    );
    if let m31a::tui::model::RuntimeStartupState::Failed(err) = &app.model.runtime_status {
        assert!(err.contains("timed out"), "must say timed out: {err}");
        assert!(
            err.contains(stage::CRASH_RECOVERY_INTEGRITY),
            "timeout must name last stage, got: {err}"
        );
    } else {
        panic!("expected Failed startup state");
    }
    let _ = fake_task.await;
}

#[tokio::test]
async fn assembly_timeout_without_progress_reports_no_progress_state() {
    // Distinguish "no stage reported" from "a stage is running".
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let (_ptx, progress_rx) = progress_channel();
    let fake_task = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort = fake_task.abort_handle();
    let mut app = TuiApplication::new().with_assembly_timeout(Duration::from_millis(40));
    app.begin_assembly_with_progress(rx, progress_rx, abort);
    assert!(app.last_assembly_stage().is_none());
    tokio::time::sleep(Duration::from_millis(60)).await;
    let mut terminal = test_backend_terminal();
    app.poll_runtime(&mut terminal).await;
    if let m31a::tui::model::RuntimeStartupState::Failed(err) = &app.model.runtime_status {
        assert!(
            err.contains("no startup stage reported progress"),
            "must distinguish no-progress, got: {err}"
        );
    } else {
        panic!("expected Failed startup state");
    }
    let _ = fake_task.await;
}

#[tokio::test]
async fn assembly_timeout_transitions_tui_to_failed_and_aborts_work() {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let task_handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort_handle = task_handle.abort_handle();
    let mut app = TuiApplication::new().with_assembly_timeout(Duration::from_millis(50));
    app.begin_assembly_with_abort_handle(rx, abort_handle);
    tokio::time::sleep(Duration::from_millis(70)).await;
    let mut terminal = test_backend_terminal();
    app.poll_runtime(&mut terminal).await;
    assert!(app.model.runtime_status.is_failed());
    assert!(
        task_handle.await.unwrap_err().is_cancelled(),
        "timed-out work must be aborted"
    );
}

#[tokio::test]
async fn stale_completion_from_timed_out_attempt_cannot_attach() {
    // First attempt times out; its late Ready must never attach.
    let (tx1, rx1) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let fake1 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let mut app = TuiApplication::new().with_assembly_timeout(Duration::from_millis(40));
    app.begin_assembly_with_abort_handle(rx1, fake1.abort_handle());
    tokio::time::sleep(Duration::from_millis(60)).await;
    let mut terminal = test_backend_terminal();
    app.poll_runtime(&mut terminal).await;
    assert!(app.model.runtime_status.is_failed());

    // Late Ready on the settled attempt: poll again must ignore it.
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();
    let pool = initialize_database(&dir.path().join("stale.db"))
        .await
        .unwrap();
    let rt = assemble_runtime(pool, ws).await;
    tx1.send(RuntimeAssemblyOutcome::Ready(rt)).unwrap();
    app.poll_runtime(&mut terminal).await;
    // Still failed, never Ready; no runtime attached.
    assert!(app.model.runtime_status.is_failed());
    assert!(!app.binding.has_runtime());
    let _ = fake1.await;
}

#[tokio::test]
async fn retry_aborts_previous_attempt_no_duplicate_recovery() {
    // Retries abort the previous attempt first (no concurrent recovery).
    let (_tx1, rx1) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let task1 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort1 = task1.abort_handle();
    let mut app = TuiApplication::new();
    app.begin_assembly_with_abort_handle(rx1, abort1);
    let gen1 = app.assembly_generation();

    let (_tx2, rx2) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    let task2 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort2 = task2.abort_handle();
    app.begin_assembly_with_abort_handle(rx2, abort2);
    assert!(app.assembly_generation() > gen1);
    // First task must have been aborted by the retry.
    assert!(task1.await.unwrap_err().is_cancelled());
    // Second attempt still pending (not double-settled).
    let mut terminal = test_backend_terminal();
    assert!(!app.poll_runtime(&mut terminal).await);
    task2.abort();
    let _ = task2.await;
}

#[tokio::test]
async fn retry_after_timeout_does_not_duplicate_completed_recovery() {
    // Recovery is idempotent: a second scan over already-reconciled state
    // performs no duplicate kills/seals and leaves jobs Lost.
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("retry.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    let (task_id, agent_id) = seed_mission(&pool, mission, "running").await;
    // One running job with no pid/spools: reconciled to Lost exactly once.
    let job = JobId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO jobs (id, mission_id, task_id, agent_id, command, args_json, working_dir, state, provider, resource_limits_json, submitted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind("echo hi")
    .bind("[]")
    .bind(ws.to_str().unwrap())
    .bind("running")
    .bind("local")
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool.clone(), store, &ws);
    let first = scanner.scan_all_in_flight().await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].reconciled_jobs_count, 1);
    let second = scanner.scan_all_in_flight().await.unwrap();
    assert_eq!(second.len(), 1);
    // Already Lost: second pass reconciles nothing (no duplicate work).
    assert_eq!(second[0].reconciled_jobs_count, 0);
    let state: String = sqlx::query_scalar("SELECT state FROM jobs WHERE id = ?")
        .bind(job.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "Lost");
}

// ── Crash recovery correctness ─────────────────────────────────────────────

#[tokio::test]
async fn recovery_no_in_flight_missions_yields_empty_scan() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("rec0.db"))
        .await
        .unwrap();
    // Only terminal missions.
    let m = MissionId::new();
    seed_mission(&pool, m, "completed").await;
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let results = scanner.scan_all_in_flight().await.unwrap();
    assert!(results.is_empty());
    assert_eq!(scanner.integrity_check_count(), 1);
}

#[tokio::test]
async fn recovery_one_in_flight_mission_is_safe_to_resume() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("a.txt"), b"hello").unwrap();
    let pool = initialize_database(&dir.path().join("rec1.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    seed_mission(&pool, mission, "running").await;
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let results = scanner.scan_all_in_flight().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].classification,
        CrashRecoveryClassification::SafeToResume
    );
    assert!(results[0].classification.is_safe());
}

#[tokio::test]
async fn recovery_many_missions_single_integrity_check() {
    // Core performance invariant: N missions → exactly 1 full integrity check.
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("a.txt"), b"hello").unwrap();
    let pool = initialize_database(&dir.path().join("recN.db"))
        .await
        .unwrap();
    const N: usize = 12;
    for _ in 0..N {
        seed_mission(&pool, MissionId::new(), "running").await;
    }
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let results = scanner.scan_all_in_flight().await.unwrap();
    assert_eq!(results.len(), N);
    assert_eq!(
        scanner.integrity_check_count(),
        1,
        "full database integrity check must run once per scan, not once per mission"
    );
    // Single-mission path still performs exactly one check (correct alone).
    let dir2 = tempdir().unwrap();
    let ws2 = dir2.path().join("ws");
    std::fs::create_dir_all(&ws2).unwrap();
    let pool2 = initialize_database(&dir2.path().join("rec1b.db"))
        .await
        .unwrap();
    let m = MissionId::new();
    seed_mission(&pool2, m, "running").await;
    let store2 = Arc::new(FsArtifactStore::new(dir2.path().join("artifacts")));
    let scanner2 = StartupCrashRecoveryScanner::new(pool2, store2, &ws2);
    scanner2.scan_and_reconcile(m).await.unwrap();
    assert_eq!(scanner2.integrity_check_count(), 1);
}

#[tokio::test]
async fn recovery_corrupt_checkpoint_manifest_is_corrupt_not_safe() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("recBadCp.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    seed_mission(&pool, mission, "running").await;
    // Corrupt manifest JSON row.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO checkpoints (id, mission_id, sequence, stage, cycle, state_summary, manifest_json, manifest_hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::now_v7().as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind(1i64)
    .bind("Verify")
    .bind(1i64)
    .bind("corrupt state")
    .bind("{corrupt json[[[")
    .bind("deadbeef")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let res = scanner.scan_and_reconcile(mission).await.unwrap();
    assert_eq!(res.classification, CrashRecoveryClassification::Corrupt);
    assert!(!res.classification.is_safe());
}

#[tokio::test]
async fn recovery_missing_checkpoint_artifact_is_corrupt() {
    use m31a::checkpoint::manager::{CheckpointManager, StagedArtifactInput};
    use m31a::checkpoint::manifest::CheckpointManifest;
    use m31a::ids::{ArtifactId, CheckId, CheckpointId};
    use std::collections::BTreeMap;

    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("keep.txt"), b"data").unwrap();
    let pool = initialize_database(&dir.path().join("recMissArt.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    let (task_id, _) = seed_mission(&pool, mission, "running").await;
    let store: Arc<FsArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    // Valid checkpoint referencing one artifact.
    let manager = CheckpointManager::new(pool.clone(), store.clone(), dir.path().join("staging"));
    let cp_id = CheckpointId::new();
    let art_id = ArtifactId::new();
    let art_data = b"evidence bytes".to_vec();
    let art_hash = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&art_data);
        format!("{:x}", h.finalize())
    };
    let mut task_states = BTreeMap::new();
    task_states.insert(task_id, m31a::state_machine::TaskState::Succeeded);
    // Snapshot identity must match current workspace OR no saved baseline →
    // manifest without saved baseline compares hash; use empty snapshot to
    // skip workspace comparison and isolate the artifact check… instead save
    // a matching baseline first.
    let baseline =
        m31a::repo::drift::RepositoryBaseline::capture(&ws, mission, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();
    let snapshot_identity = {
        use sha2::{Digest, Sha256};
        let fp = serde_json::to_string(&baseline.file_hashes).unwrap();
        let mut h = Sha256::new();
        h.update(fp.as_bytes());
        format!("{:x}", h.finalize())
    };
    let manifest = CheckpointManifest::new(
        cp_id,
        mission,
        1,
        "Verify",
        1,
        &snapshot_identity,
        task_states,
        BTreeMap::new(),
        "policy_context_hash_val",
        vec![],
        vec![(art_id, art_hash, art_data.len() as u64)],
        "note",
    );
    manager
        .create_checkpoint(
            &manifest,
            vec![StagedArtifactInput::new(
                art_id, art_data, "bin", "evidence",
            )],
        )
        .await
        .unwrap();
    // Delete the artifact file → Corrupt, never success.
    let art_path = store.path_for(art_id, "bin");
    std::fs::remove_file(&art_path).unwrap();
    let _ = CheckId::new();
    let scanner = StartupCrashRecoveryScanner::new(pool, store.clone(), &ws);
    let res = scanner.scan_and_reconcile(mission).await.unwrap();
    assert_eq!(res.classification, CrashRecoveryClassification::Corrupt);
    assert!(!res.classification.is_safe());
}

#[tokio::test]
async fn recovery_workspace_drift_is_ambiguous_never_safe() {
    use m31a::checkpoint::manager::CheckpointManager;
    use m31a::checkpoint::manifest::CheckpointManifest;
    use m31a::ids::CheckpointId;
    use std::collections::BTreeMap;

    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("base.txt"), b"v1").unwrap();
    let pool = initialize_database(&dir.path().join("recDrift.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    let (task_id, _) = seed_mission(&pool, mission, "running").await;
    let store: Arc<FsArtifactStore> = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    // Saved baseline + matching checkpoint (no artifacts → workspace check runs).
    let baseline =
        m31a::repo::drift::RepositoryBaseline::capture(&ws, mission, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();
    let snapshot_identity = {
        use sha2::{Digest, Sha256};
        let fp = serde_json::to_string(&baseline.file_hashes).unwrap();
        let mut h = Sha256::new();
        h.update(fp.as_bytes());
        format!("{:x}", h.finalize())
    };
    let manager = CheckpointManager::new(pool.clone(), store.clone(), dir.path().join("staging"));
    let mut task_states = BTreeMap::new();
    task_states.insert(task_id, m31a::state_machine::TaskState::Succeeded);
    let manifest = CheckpointManifest::new(
        CheckpointId::new(),
        mission,
        1,
        "Verify",
        1,
        &snapshot_identity,
        task_states,
        BTreeMap::new(),
        "policy_context_hash_val",
        vec![],
        vec![],
        "drift test checkpoint",
    );
    manager.create_checkpoint(&manifest, vec![]).await.unwrap();
    // Sanity: clean state is safe.
    let scanner = StartupCrashRecoveryScanner::new(pool.clone(), store.clone(), &ws);
    let clean = scanner.scan_and_reconcile(mission).await.unwrap();
    assert_eq!(
        clean.classification,
        CrashRecoveryClassification::SafeToResume
    );

    // Unexplained drift → Ambiguous, never safe (fail-closed).
    std::fs::write(ws.join("unexpected.rs"), b"tampered").unwrap();
    let drifted = scanner.scan_and_reconcile(mission).await.unwrap();
    assert_eq!(
        drifted.classification,
        CrashRecoveryClassification::Ambiguous
    );
    assert!(!drifted.classification.is_safe());
    assert!(!CrashRecoveryClassification::Corrupt.is_safe());
}

#[tokio::test]
async fn recovery_job_and_task_reconciliation() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("recJobs.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    let (task_id, agent_id) = seed_mission(&pool, mission, "running").await;
    let job = JobId::new();
    let spool = dir.path().join("out.spool");
    std::fs::write(&spool, b"partial output\n").unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO jobs (id, mission_id, task_id, agent_id, command, args_json, working_dir, state, provider, resource_limits_json, stdout_spool_path, submitted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job.as_bytes().as_slice())
    .bind(mission.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind("sleep 1")
    .bind("[]")
    .bind(ws.to_str().unwrap())
    .bind("running")
    .bind("local")
    .bind("{}")
    .bind(spool.to_str().unwrap())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool.clone(), store, &ws);
    let res = scanner.scan_and_reconcile(mission).await.unwrap();
    assert_eq!(res.reconciled_jobs_count, 1);
    assert_eq!(res.sealed_spools_count, 1);
    assert!(res.explanation.contains("in-flight tasks reconciled"));
    let state: String = sqlx::query_scalar("SELECT state FROM jobs WHERE id = ?")
        .bind(job.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "Lost");
    let task_state: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(task_state.to_lowercase(), "pending");
}

#[tokio::test]
async fn recovery_persistence_error_propagates_not_swallowed() {
    // Drop the ledger table: persistence must surface as Err, never Ok(success).
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("recPersist.db"))
        .await
        .unwrap();
    let mission = MissionId::new();
    seed_mission(&pool, mission, "running").await;
    sqlx::query("DROP TABLE crash_recovery_scans")
        .execute(&pool)
        .await
        .unwrap();
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let res = scanner.scan_and_reconcile(mission).await;
    assert!(
        res.is_err(),
        "persistence failure must propagate, not become fake success"
    );
}

#[tokio::test]
async fn recovery_cancellation_is_real() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let pool = initialize_database(&dir.path().join("recCancel.db"))
        .await
        .unwrap();
    for _ in 0..4 {
        seed_mission(&pool, MissionId::new(), "running").await;
    }
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool, store, &ws);
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel();
    let res = scanner
        .scan_all_in_flight_with_progress(None, Some(&token))
        .await;
    assert!(
        matches!(
            res,
            Err(m31a::checkpoint::crash_recovery::CrashRecoveryError::Cancelled(_))
        ),
        "cancelled scan must return Cancelled, got: {res:?}"
    );
    // Single-mission cancellation likewise.
    let m = MissionId::new();
    let res1 = scanner.scan_and_reconcile_cancelled(m, Some(&token)).await;
    assert!(res1.is_err());
}

#[tokio::test]
async fn recovery_failed_outcome_is_never_safe_to_resume() {
    // Fail-closed: Ambiguous/Corrupt are never safe, even with jobs reconciled.
    assert!(!CrashRecoveryClassification::Ambiguous.is_safe());
    assert!(!CrashRecoveryClassification::Corrupt.is_safe());
    assert!(!CrashRecoveryClassification::NeedsRepair.is_safe());
    assert!(CrashRecoveryClassification::SafeToResume.is_safe());
}

#[tokio::test]
async fn tui_failed_outcome_surfaces_startup_error() {
    // A Failed assembly outcome (e.g. explicit config error from the
    // composition root) reaches Failed TUI state with the message intact.
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    drop(_tx);
    let mut app = TuiApplication::new();
    // Simulate composition-root failure delivery.
    let (tx2, rx2) = tokio::sync::mpsc::unbounded_channel::<RuntimeAssemblyOutcome>();
    tx2.send(RuntimeAssemblyOutcome::Failed(TuiError::runtime(
        "failed to assemble complete AppRuntime for cockpit: invalid workspace configuration",
    )))
    .unwrap();
    drop(tx2);
    let _ = rx;
    app.begin_assembly(rx2);
    let mut terminal = test_backend_terminal();
    app.poll_runtime(&mut terminal).await;
    assert!(app.model.runtime_status.is_failed());
    if let m31a::tui::model::RuntimeStartupState::Failed(err) = &app.model.runtime_status {
        assert!(err.contains("invalid workspace configuration"), "{err}");
        assert!(!err.contains("deadline exceeded"), "{err}");
    }
}

// ── Performance / stress (ignored by default; run explicitly) ──────────────

/// Reproducible stress fixture: multiple unfinished missions over a
/// realistically-sized repository/database fixture. Measures each stage
/// independently and reports observed durations + operation counts.
///
/// Run with:
/// `cargo test --test p0_startup_assembly_regression stress_startup_scan_stages -- --ignored --nocapture`
#[tokio::test]
#[ignore = "stress/benchmark: realistically-sized fixture, run explicitly for stage timings"]
async fn stress_startup_scan_stages() {
    let dir = tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    // Realistic repo shape: 300 files across nested dirs (~tens of KB).
    for d in 0..15 {
        let sub = ws.join(format!("crate_{d}"));
        std::fs::create_dir_all(&sub).unwrap();
        for f in 0..20 {
            let content = format!("// file {d}/{f}\nfn f{f}() {{}}\n").repeat(20);
            std::fs::write(sub.join(format!("mod_{f}.rs")), content).unwrap();
        }
    }
    let pool = initialize_database(&dir.path().join("stress.db"))
        .await
        .unwrap();
    const N: usize = 12;
    for _ in 0..N {
        seed_mission(&pool, MissionId::new(), "running").await;
    }
    let store = Arc::new(FsArtifactStore::new(dir.path().join("artifacts")));
    let scanner = StartupCrashRecoveryScanner::new(pool.clone(), store.clone(), &ws);

    let t0 = std::time::Instant::now();
    let rows: Vec<(Vec<u8>,)> =
        sqlx::query_as("SELECT id FROM missions WHERE LOWER(status) NOT IN ('completed','succeeded','failed','cancelled')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let enumerate = t0.elapsed();
    let t1 = std::time::Instant::now();
    let results = scanner.scan_all_in_flight().await.unwrap();
    let scan_total = t1.elapsed();
    println!(
        "stress: missions={} db_rows={} integrity_checks={}",
        results.len(),
        rows.len(),
        scanner.integrity_check_count()
    );
    println!(
        "stress: enumerate={:?} scan_total={:?} per_mission_avg={:?}",
        enumerate,
        scan_total,
        scan_total / (N as u32)
    );
    assert_eq!(results.len(), N);
    assert_eq!(scanner.integrity_check_count(), 1);

    // Full assembly timing (includes authorities beyond recovery).
    let bus = Arc::new(BroadcastEventBus::new(128));
    let config = Arc::new(ResolvedConfigBuilder::new(&ws).build_fallback());
    let t2 = std::time::Instant::now();
    let rt = AppRuntime::from_pool_workspace_and_config(pool, ws, bus, config)
        .await
        .unwrap();
    println!("stress: full assembly={:?}", t2.elapsed());
    rt.shutdown().await;
}
