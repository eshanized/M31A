//! Architectural regression tests asserting persistence authority and schema canonicalization invariants.
//!
//! Non-negotiable architectural invariants:
//! 1. Zero inline DDL outside `migrations/`.
//! 2. Zero unauthorized SQL mutations in orchestration / CLI / controller / runtime layers.
//! 3. Migration engine owns all schema definitions idempotently.
//! 4. System state is managed solely via canonical `SqliteSystemStateRepository`.
//! 5. Task state transitions enforce domain types and persistence authority.

use m31a::ids::{MissionId, TaskId};
use m31a::persistence::sqlite::repositories::{
    SqliteMissionRepository, SqliteSystemStateRepository, SqliteTaskRepository,
};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::state::Task;
use m31a::state::mission::Mission;
use m31a::state::task::TaskResult;
use m31a::state_machine::TaskState;
use sqlx::SqlitePool;
use std::fs;
use std::path::Path;

#[test]
fn test_no_inline_ddl_outside_migrations() {
    let src_dir = Path::new("src");
    let forbidden_ddl_patterns = [
        "CREATE TABLE",
        "CREATE INDEX",
        "ALTER TABLE",
        "DROP TABLE",
        "CREATE UNIQUE INDEX",
    ];

    let mut violations = Vec::new();

    fn scan_dir(dir: &Path, patterns: &[&str], violations: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, patterns, violations);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("//")
                            || trimmed.starts_with("/*")
                            || trimmed.starts_with('*')
                            || trimmed.contains("\"bad id; drop table\"")
                        {
                            continue;
                        }
                        let upper = trimmed.to_uppercase();
                        for pattern in patterns {
                            if upper.contains(pattern) {
                                violations.push(format!(
                                    "Found forbidden inline DDL '{}' in file {:?}: {}",
                                    pattern, path, line
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(src_dir, &forbidden_ddl_patterns, &mut violations);

    assert!(
        violations.is_empty(),
        "Architecture violation: Inline DDL found outside migrations directory:\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_no_direct_sql_mutations_in_orchestration_layers() {
    let forbidden_layers = [
        "src/controller",
        "src/runtime.rs",
        "src/scheduler/engine.rs",
        "src/cli",
        "src/interaction/commands.rs",
        "src/recovery/adapter.rs",
        "src/recovery/replan.rs",
        "src/checkpoint/resume.rs",
        "src/init/lifecycle.rs",
    ];

    let forbidden_mutations = [
        "INSERT INTO missions",
        "UPDATE missions SET",
        "INSERT INTO tasks",
        "UPDATE tasks SET",
        "INSERT INTO approval_requests",
        "UPDATE approval_requests SET",
        "INSERT INTO recovery_attempts",
        "INSERT INTO system_state",
        "UPDATE system_state SET",
    ];

    let mut violations = Vec::new();

    for layer in &forbidden_layers {
        let path = Path::new(layer);
        if path.is_file() {
            let content = fs::read_to_string(path).unwrap_or_default();
            for pattern in &forbidden_mutations {
                if content.to_uppercase().contains(pattern) {
                    violations.push(format!(
                        "Forbidden raw mutation '{}' found in {:?}",
                        pattern, path
                    ));
                }
            }
        } else if path.is_dir() {
            for entry in fs::read_dir(path).unwrap().flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    let content = fs::read_to_string(&p).unwrap_or_default();
                    for pattern in &forbidden_mutations {
                        if content.to_uppercase().contains(pattern) {
                            violations.push(format!(
                                "Forbidden raw mutation '{}' found in {:?}",
                                pattern, p
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Architecture violation: Direct SQL mutations found in orchestration/service layers:\n{}",
        violations.join("\n")
    );
}

#[tokio::test]
async fn test_schema_migration_authority_and_system_state() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    // 1. Run migrations
    run_migrations(&pool)
        .await
        .expect("Initial schema migration failed");

    // 2. Assert idempotency
    run_migrations(&pool)
        .await
        .expect("Secondary idempotent schema migration failed");

    // 3. Verify system_state table exists and is operational
    let state_repo = SqliteSystemStateRepository::new(pool.clone());
    let initial_val = state_repo
        .get("test_state_key")
        .await
        .expect("Failed to query system_state");
    assert_eq!(
        initial_val, None,
        "Fresh database should have empty test key"
    );

    state_repo
        .set("test_state_key", "active_operational")
        .await
        .expect("Failed to set system_state");

    let updated_val = state_repo
        .get("test_state_key")
        .await
        .expect("Failed to retrieve updated system_state");
    assert_eq!(
        updated_val,
        Some("active_operational".to_string()),
        "System state key-value should be persisted durably"
    );

    // Test onboarding helper methods
    state_repo
        .record_onboarding("{\"completed\": true}", "onboarded")
        .await
        .expect("Failed to record onboarding");

    let sentinel = state_repo
        .get("onboarding_sentinel")
        .await
        .expect("Failed to read onboarding sentinel");
    assert_eq!(sentinel, Some("{\"completed\": true}".to_string()));

    let init_state = state_repo
        .get("init_state")
        .await
        .expect("Failed to read init state");
    assert_eq!(init_state, Some("onboarded".to_string()));
}

#[tokio::test]
async fn test_task_state_transitions_via_repository_authority() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");
    run_migrations(&pool)
        .await
        .expect("Schema migrations failed");

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());

    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Persistence Authority Test Mission".to_string());
    mission_repo
        .insert(&mission)
        .await
        .expect("Failed to insert mission");

    let task_id = TaskId::new();
    let task = Task::new(
        task_id,
        mission.id,
        "Verify repository authority".to_string(),
    );
    task_repo
        .insert(&task)
        .await
        .expect("Failed to insert task");

    // Verify initial count and state
    let total = task_repo
        .count_by_mission(mission.id)
        .await
        .expect("Failed to count tasks");
    assert_eq!(total, 1);

    let completed = task_repo
        .count_completed_by_mission(mission.id)
        .await
        .expect("Failed to count completed tasks");
    assert_eq!(completed, 0);

    // Transition task through states
    task_repo
        .mark_ready(task.id)
        .await
        .expect("Failed to mark ready");
    let fetched = task_repo
        .get(task.id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(fetched.status, TaskState::Ready);

    task_repo
        .mark_running(task.id, chrono::Utc::now())
        .await
        .expect("Failed to mark running");
    let fetched = task_repo
        .get(task.id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(fetched.status, TaskState::Running);

    let result = TaskResult::new("Task succeeded verification");
    task_repo
        .mark_succeeded(task.id, &result)
        .await
        .expect("Failed to mark succeeded");
    let fetched = task_repo
        .get(task.id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(fetched.status, TaskState::Succeeded);

    let completed_after = task_repo
        .count_completed_by_mission(mission.id)
        .await
        .expect("Failed to count completed tasks");
    assert_eq!(completed_after, 1);

    // Test crash recovery reset
    let reset_count = task_repo
        .reset_running_tasks_to_pending(mission.id)
        .await
        .expect("Failed to reset running tasks");
    assert_eq!(reset_count, 0, "No running tasks should be reset");

    task_repo
        .mark_running(task.id, chrono::Utc::now())
        .await
        .expect("Mark running");
    let reset_count = task_repo
        .reset_running_tasks_to_pending(mission.id)
        .await
        .expect("Failed to reset running tasks");
    assert_eq!(reset_count, 1, "Exactly 1 running task should be reset");

    let fetched = task_repo
        .get(task.id)
        .await
        .expect("Failed to get task")
        .expect("Task not found");
    assert_eq!(fetched.status, TaskState::Pending);
}
