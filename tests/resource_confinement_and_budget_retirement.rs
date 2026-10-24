//! Phase 12 Confinement & Budget Decommissioning Verification Suite (BST-01, BST-02, SEC-03).
//!
//! Validates:
//! 1. OS-level process confinement (rlimits) via `crate::platform::resources::ResourceBudget` remains intact.
//! 2. Streaming artifact quota enforcement remains operational as an independent security control.
//! 3. Database initialization and migration run cleanly without budget tables or columns.
//! 4. Mission creation and storage operate without any budget enforcer or grant dependencies.
//! 5. Legacy `[budget]` configuration sections parse cleanly without crashing the runtime.

use tempfile::tempdir;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use m31a::ids::{JobId, MissionId};
use m31a::persistence::artifacts::quota::{ArtifactExemption, QuotaEnforcer};
use m31a::persistence::sqlite::repositories::SqliteMissionRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::platform::resources::ResourceBudget as PlatformResourceBudget;
use m31a::process::confinement::ConfinementManager;
use m31a::state::mission::Mission;

#[test]
fn test_platform_resource_budget_struct() {
    let budget = PlatformResourceBudget {
        max_cpu_seconds: Some(5),
        max_memory_bytes: Some(1024 * 1024 * 512),
        max_processes: Some(32),
        max_open_files: Some(1024),
        max_output_bytes: Some(1024 * 1024 * 100),
    };

    assert_eq!(budget.max_cpu_seconds, Some(5));
    assert_eq!(budget.max_memory_bytes, Some(536870912));
}

#[tokio::test]
async fn test_process_confinement_rlimits() {
    let manager = ConfinementManager::new();
    let mut cmd = Command::new("true");
    let limits = PlatformResourceBudget {
        max_cpu_seconds: Some(5),
        max_memory_bytes: Some(1024 * 1024 * 256), // 256 MB
        max_processes: None,
        max_open_files: None,
        max_output_bytes: None,
    };
    let job_id = JobId::new();

    let mut handle = manager.apply_confinement(&mut cmd, &limits, &job_id);

    // Spawn and run process
    let mut child = cmd.spawn().expect("Spawn confined command");
    if let Some(pid) = child.id() {
        handle.attach_pid(pid);
    }
    let status = child.wait().await.expect("Wait on child");
    assert!(status.success());
    handle.cleanup();
}

#[tokio::test]
async fn test_streaming_artifact_quota_enforcement() {
    let enforcer = QuotaEnforcer::new(Some(50), Some(200));

    // 1. Standard write within limits succeeds
    let mut buffer = Vec::new();
    let mut writer = enforcer.wrap_writer(&mut buffer, ArtifactExemption::Standard);
    writer
        .write_all(b"small payload under 50 bytes")
        .await
        .unwrap();
    assert_eq!(writer.bytes_written(), 28);
    assert_eq!(enforcer.current_cumulative_bytes(), 28);

    // 2. Standard write exceeding single artifact limit is aborted immediately
    let mut big_buffer = Vec::new();
    let mut big_writer = enforcer.wrap_writer(&mut big_buffer, ArtifactExemption::Standard);
    let chunk = [1u8; 30];
    big_writer.write_all(&chunk).await.unwrap();
    let err = big_writer.write_all(&chunk).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::FileTooLarge);

    // 3. Protected artifact (verification / report) is exempt from limit
    let mut protected_buffer = Vec::new();
    let mut protected_writer =
        enforcer.wrap_writer(&mut protected_buffer, ArtifactExemption::Protected);
    let large_evidence = [7u8; 150];
    assert!(protected_writer.write_all(&large_evidence).await.is_ok());
    assert!(!enforcer.can_evict(ArtifactExemption::Protected));
}

#[tokio::test]
async fn test_database_and_mission_lifecycle_without_budgets() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mission_no_budget_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    // Verify budget_grants and budget_ledger tables do not exist
    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('budget_grants', 'budget_ledger')"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 0, "Budget tables must not exist in schema");

    let repo = SqliteMissionRepository::new(pool);
    let mission_id = MissionId::new();
    let mission = Mission::new(
        mission_id,
        "Implement feature without budget restriction".to_string(),
    );

    repo.insert(&mission).await.unwrap();

    let fetched = repo.get(mission_id).await.unwrap().expect("mission exists");
    assert_eq!(fetched.id, mission_id);
    assert_eq!(
        fetched.objective,
        "Implement feature without budget restriction"
    );
}

#[test]
fn test_legacy_budget_config_parsing_backward_compatibility() {
    let legacy_toml = r#"
        [workspace]
        project_type = "rust"

        [budget]
        max_tokens = 5000000
        max_cost_usd = 50.0
        max_agent_steps = 100
    "#;

    let parsed: Result<m31a::config::schema::AppConfig, _> = toml::from_str(legacy_toml);
    assert!(
        parsed.is_ok(),
        "Legacy [budget] section in config must be ignored gracefully without errors: {:?}",
        parsed.err()
    );
}
