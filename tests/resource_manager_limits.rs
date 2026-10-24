//! Phase 05: Hierarchical Resource Locking and ResourceManager Integration Tests
//!
//! Verifies:
//! 1. Disjoint resource concurrency (shared and distinct paths).
//! 2. Subtree vs Exact conflict matrix enforcement.
//! 3. Deadlock prevention via deterministic global key ordering.
//! 4. Automatic RAII drop cleanup releasing held locks.
//! 5. SQLite durable lease tracking with owner generations and release timestamps.

use m31a::ids::{MissionId, TaskId};
use m31a::persistence::initialize_database;
use m31a::persistence::sqlite::repositories::SqliteMissionRepository;
use m31a::scheduler::resources::{LockMode, PathScope, ResourceKey, ResourceManager};
use m31a::state::mission::Mission;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_concurrency_disjoint_resources() {
    let manager = ResourceManager::new(None, 1);
    let mission_id = MissionId::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();

    let res_1 = ResourceKey::workspace("src/dag", PathScope::Subtree);
    let res_2 = ResourceKey::workspace("src/planning", PathScope::Subtree);

    let guard_1 = manager
        .acquire(task_1, mission_id, vec![(res_1, LockMode::Exclusive)])
        .expect("Task 1 should acquire src/dag");
    let guard_2 = manager
        .acquire(task_2, mission_id, vec![(res_2, LockMode::Exclusive)])
        .expect("Task 2 should acquire src/planning concurrently");

    assert_eq!(manager.active_leases_count(), 2);
    drop(guard_1);
    drop(guard_2);
    assert_eq!(manager.active_leases_count(), 0);
}

#[tokio::test]
async fn test_subtree_conflict_with_exact_descendant() {
    let manager = ResourceManager::new(None, 1);
    let mission_id = MissionId::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();

    let tree_lock = ResourceKey::workspace("src", PathScope::Subtree);
    let file_lock = ResourceKey::workspace("src/dag/graph.rs", PathScope::Exact);

    let _guard_1 = manager
        .acquire(task_1, mission_id, vec![(tree_lock, LockMode::Exclusive)])
        .expect("Task 1 acquires src/** exclusively");

    // Task 2 attempts to acquire file under src/**
    let conflict = manager
        .acquire(
            task_2,
            mission_id,
            vec![(file_lock.clone(), LockMode::Exclusive)],
        )
        .expect_err("Task 2 must be rejected with conflict");

    assert_eq!(conflict.held_by_task, task_1);
    assert_eq!(conflict.requested, file_lock);
}

#[tokio::test]
async fn test_deadlock_prevention_reverse_key_orders() {
    let manager = Arc::new(ResourceManager::new(None, 1));
    let mission_id = MissionId::new();

    let res_a = ResourceKey::workspace("src/a.rs", PathScope::Exact);
    let res_b = ResourceKey::workspace("src/b.rs", PathScope::Exact);

    // Spawn 20 concurrent contending tasks requesting [A, B] and [B, A]
    let mut handles = Vec::new();
    for i in 0..20 {
        let mgr = manager.clone();
        let ra = res_a.clone();
        let rb = res_b.clone();

        handles.push(tokio::spawn(async move {
            let task_id = TaskId::new();
            let reqs = if i % 2 == 0 {
                vec![(ra, LockMode::Exclusive), (rb, LockMode::Exclusive)]
            } else {
                vec![(rb, LockMode::Exclusive), (ra, LockMode::Exclusive)]
            };

            // Attempt acquisition (will either succeed atomically or fail immediately with conflict)
            match mgr.acquire(task_id, mission_id, reqs) {
                Ok(guard) => {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    drop(guard);
                    true
                }
                Err(_) => false,
            }
        }));
    }

    let mut succeeded = 0;
    for h in handles {
        if h.await.unwrap() {
            succeeded += 1;
        }
    }

    // At least one task succeeded, and all tasks completed without deadlock
    assert!(succeeded >= 1);
    assert_eq!(manager.active_leases_count(), 0);
}

#[tokio::test]
async fn test_raii_cleanup_frees_held_locks() {
    let manager = ResourceManager::new(None, 1);
    let mission_id = MissionId::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();

    let res = ResourceKey::workspace("src/lib.rs", PathScope::Exact);

    {
        let guard_1 = manager
            .acquire(task_1, mission_id, vec![(res.clone(), LockMode::Exclusive)])
            .expect("Task 1 acquires src/lib.rs");

        // While guard_1 is in scope, Task 2 is rejected
        assert!(
            manager
                .acquire(task_2, mission_id, vec![(res.clone(), LockMode::Exclusive)])
                .is_err()
        );
        drop(guard_1);
    }

    // After guard_1 is dropped, Task 2 acquires immediately
    let guard_2 = manager
        .acquire(task_2, mission_id, vec![(res.clone(), LockMode::Exclusive)])
        .expect("Task 2 acquires after RAII drop");

    assert_eq!(manager.active_leases_count(), 1);
    drop(guard_2);
    assert_eq!(manager.active_leases_count(), 0);
}

#[tokio::test]
async fn test_durable_lease_persistence_sqlite() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("resource_leases_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Resource Lease Test Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let task_id = TaskId::new();
    // Insert dummy task so foreign key succeeds
    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, mission_id, title, role, status, priority, max_retries, retry_count,
            capabilities, verification, estimates, required_resources, fingerprint,
            created_at, updated_at
        ) VALUES (?, ?, 'Resource Task', 'implementer', 'running', 100, 3, 0, '[]', '{}', '{}', '[]', '', ?, ?)
        "#,
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    let manager = ResourceManager::new(Some(pool.clone()), 42);
    let res = ResourceKey::workspace("src/kernel", PathScope::Subtree);

    // 1. Acquire persisted lease
    let guard = manager
        .acquire_persisted(
            task_id,
            mission_id,
            vec![(res.clone(), LockMode::Exclusive)],
        )
        .await
        .expect("Acquire persisted");

    // Verify row in SQLite resource_leases
    let row = sqlx::query(
        r#"
        SELECT resource_key, lock_mode, scope, owner_generation, released_at
        FROM resource_leases
        WHERE task_id = ?
        "#,
    )
    .bind(task_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();

    use sqlx::Row;
    let db_key: String = row.get("resource_key");
    let db_mode: String = row.get("lock_mode");
    let db_scope: String = row.get("scope");
    let db_gen: i64 = row.get("owner_generation");
    let db_released: Option<String> = row.get("released_at");

    assert_eq!(db_key, "workspace:src/kernel/**");
    assert_eq!(db_mode, "Exclusive");
    assert_eq!(db_scope, "Subtree");
    assert_eq!(db_gen, 42);
    assert!(db_released.is_none());

    // 2. Disarm guard and perform release_task_leases_persisted
    guard.disarm();
    let released = manager.release_task_leases_persisted(task_id).await;
    assert_eq!(released.len(), 1);

    // Verify released_at is now updated in SQLite
    let updated_row = sqlx::query(
        r#"
        SELECT released_at
        FROM resource_leases
        WHERE task_id = ?
        "#,
    )
    .bind(task_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();

    let updated_released: Option<String> = updated_row.get("released_at");
    assert!(updated_released.is_some());
}
