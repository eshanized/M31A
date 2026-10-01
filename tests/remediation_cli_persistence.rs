//! Remediation Test Suite: CLI Dispatch Persistence Integration (BLK-01, CLI-01, CLI-04).
//!
//! Verifies:
//! 1. `CliDispatcher::dispatch` with `RunMission` writes a durable row to SQLite.
//! 2. `ListMissions` reads persisted rows rather than returning hardcoded empty literals.
//! 3. `ShowMission` retrieves accurate mission metadata from SQLite.
//! 4. `PauseMission`, `ResumeMission`, and `CancelMission` durably mutate SQLite status.
//! 5. `ListCheckpoints` and `RestoreCheckpoint` read from the SQLite `checkpoints` table.

use std::sync::Arc;
use tempfile::tempdir;

use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{CheckpointId, MissionId};
use m31a::persistence::sqlite::schema::initialize_database;

#[tokio::test]
async fn test_cli_dispatch_mission_lifecycle_persistence() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("cli_persistence.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    let raw_runtime = m31a::runtime::AppRuntime::from_pool_and_workspace(
        pool.clone(),
        storage_root.clone(),
        bus.clone(),
    )
    .await
    .unwrap();
    let runtime = Arc::new(raw_runtime.without_model_caller());
    let dispatcher =
        CliDispatcher::production(pool.clone(), storage_root.clone(), bus).with_runtime(runtime);

    // 1. Run mission with injected failure -> fails explicitly at
    // planning (Phase 27: no fabricated tasks), BUT the mission row is
    // still durably recorded (insert precedes planning), so the lifecycle
    // below exercises real persistence either way.
    let run_cmd = RuntimeCommand::RunMission {
        prompt: "Build autonomous spacecraft controller".to_string(),
        profile: Some("safe".to_string()),
        wait_for_approval: false,
    };
    let run_err = dispatcher
        .dispatch(run_cmd)
        .await
        .expect_err("mission without a model must fail explicitly");
    let run_err_str = run_err.to_string();
    assert!(
        run_err_str.contains("no fallback tasks substituted")
            || run_err_str.contains("No model provider")
            || run_err_str.contains("Injected deterministic model failure")
            || run_err_str.contains("AutonomyController failed")
            || run_err_str.contains("execution failed"),
        "RunMission must fail with an explicit error, got: {}",
        run_err_str
    );

    // The mission row was still durably stored before planning ran.
    let list_probe = RuntimeCommand::ListMissions { all: true };
    let probe_out = dispatcher.dispatch(list_probe).await.unwrap();
    let probe_missions = probe_out.data["missions"].as_array().unwrap();
    // Phase 29: greenfield auto-missions run genesis research as its own
    // mission row when enabled, so assert membership (main mission present),
    // not an exact row count.
    let main_mission = probe_missions
        .iter()
        .find(|m| {
            m["objective"]
                .as_str()
                .unwrap_or_default()
                .contains("spacecraft controller")
        })
        .expect("main mission row must be listed");
    let mission_id_str = main_mission["id"].as_str().unwrap().to_string();
    let mission_id: MissionId = mission_id_str.parse().unwrap();

    // Verify row exists directly in SQLite
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(exists, 1, "Mission must be durably stored in SQLite");

    // 2. List missions -> returns persisted mission. The main mission is
    // located by objective: genesis research may contribute its own
    // mission row (Phase 29), so no exact row count is asserted.
    let list_cmd = RuntimeCommand::ListMissions { all: true };
    let list_out = dispatcher.dispatch(list_cmd).await.unwrap();
    let missions = list_out.data["missions"].as_array().unwrap();
    let main = missions
        .iter()
        .find(|m| {
            m["objective"]
                .as_str()
                .unwrap_or_default()
                .contains("spacecraft controller")
        })
        .expect("main mission listed");
    assert_eq!(main["id"], mission_id_str);
    assert_eq!(main["objective"], "Build autonomous spacecraft controller");

    // 3. Show mission -> returns detailed persisted metadata
    let show_cmd = RuntimeCommand::ShowMission {
        id: mission_id_str.clone(),
    };
    let show_out = dispatcher.dispatch(show_cmd).await.unwrap();
    assert_eq!(show_out.data["mission_id"], mission_id_str);
    assert_eq!(
        show_out.data["objective"],
        "Build autonomous spacecraft controller"
    );

    // 4. Pause mission -> updates database status
    let pause_cmd = RuntimeCommand::PauseMission {
        id: mission_id_str.clone(),
    };
    dispatcher.dispatch(pause_cmd).await.unwrap();
    let state = sqlx::query_scalar::<_, String>("SELECT status FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "Paused");

    // 5. Resume mission -> updates database status
    let resume_cmd = RuntimeCommand::ResumeMission {
        id: mission_id_str.clone(),
    };
    dispatcher.dispatch(resume_cmd).await.unwrap();
    let state = sqlx::query_scalar::<_, String>("SELECT status FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "Executing");

    // 6. Cancel mission -> updates database status
    let cancel_cmd = RuntimeCommand::CancelMission {
        id: mission_id_str.clone(),
        reason: Some("Mission objective changed".to_string()),
    };
    dispatcher.dispatch(cancel_cmd).await.unwrap();
    let state = sqlx::query_scalar::<_, String>("SELECT status FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "Cancelled");
}

#[tokio::test]
async fn test_cli_dispatch_checkpoint_persistence() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("cli_checkpoints.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    let dispatcher = CliDispatcher::production(pool.clone(), storage_root.clone(), bus);

    let mission_id = MissionId::new();
    let cp_id = CheckpointId::new();

    // Insert dummy mission and checkpoint directly in SQLite
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'CP Test', 'Executing', ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

    let manifest = m31a::checkpoint::manifest::CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Checkpoint",
        1,
        "snap_01",
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
        "policy_hash",
        vec![],
        vec![],
        "Snapshot 1",
    );
    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let manifest_hash = manifest.compute_manifest_hash();

    sqlx::query("INSERT INTO checkpoints (id, mission_id, sequence, stage, cycle, state_summary, manifest_json, manifest_hash, created_at) VALUES (?, ?, 1, 'Checkpoint', 1, 'Snapshot 1', ?, ?, ?)")
        .bind(cp_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(&manifest_json)
        .bind(&manifest_hash)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

    // 1. List checkpoints via CLI dispatcher
    let list_cmd = RuntimeCommand::ListCheckpoints {
        mission_id: Some(mission_id.to_string()),
    };
    let out = dispatcher.dispatch(list_cmd).await.unwrap();
    let cps = out.data["checkpoints"].as_array().unwrap();
    assert_eq!(cps.len(), 1);
    assert_eq!(cps[0]["checkpoint_id"], cp_id.to_string());

    // 2. Restore checkpoint via CLI dispatcher
    let restore_cmd = RuntimeCommand::RestoreCheckpoint {
        id: cp_id.to_string(),
    };
    let restore_out = dispatcher.dispatch(restore_cmd).await.unwrap();
    assert_eq!(restore_out.data["restored"], true);
    assert_eq!(restore_out.data["checkpoint_id"], cp_id.to_string());
}
