use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use super::key::{LockMode, PathScope, ResourceKey};
use super::lease::{ActiveLease, LeaseId, LeaseReleaser, ResourceLeaseGuard};
use crate::error::M31AError;
use crate::ids::{MissionId, TaskId};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("resource conflict on '{requested}': held by task {held_by_task} under lease {held_lease}")]
pub struct ResourceConflictError {
    pub requested: ResourceKey,
    pub held_by_task: TaskId,
    pub held_lease: LeaseId,
}

#[derive(Debug, Default)]
struct ResourceManagerInner {
    leases: BTreeMap<LeaseId, ActiveLease>,
    task_leases: BTreeMap<TaskId, Vec<LeaseId>>,
}

/// Centralized manager for hierarchical resource locking, deadlock-free acquisition,
/// RAII guard coordination, and durable SQLite tracking.
#[derive(Clone)]
pub struct ResourceManager {
    pool: Option<SqlitePool>,
    owner_generation: u64,
    inner: Arc<Mutex<ResourceManagerInner>>,
}

impl ResourceManager {
    pub fn new(pool: Option<SqlitePool>, owner_generation: u64) -> Self {
        Self {
            pool,
            owner_generation,
            inner: Arc::new(Mutex::new(ResourceManagerInner::default())),
        }
    }

    pub fn owner_generation(&self) -> u64 {
        self.owner_generation
    }

    pub fn active_leases_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.leases.len()
    }

    pub fn get_task_leases(&self, task_id: TaskId) -> Vec<ActiveLease> {
        let inner = self.inner.lock().unwrap();
        if let Some(lease_ids) = inner.task_leases.get(&task_id) {
            lease_ids
                .iter()
                .filter_map(|lid| inner.leases.get(lid).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Read-only pre-flight check without acquiring locks (for queue bypass checks).
    pub fn check_availability(
        &self,
        requests: &[(ResourceKey, LockMode)],
        requesting_task: TaskId,
    ) -> Result<(), ResourceConflictError> {
        let inner = self.inner.lock().unwrap();

        for (requested_key, requested_mode) in requests {
            for lease in inner.leases.values() {
                if lease.task_id != requesting_task
                    && requested_key.conflicts_with(*requested_mode, &lease.key, lease.mode)
                {
                    return Err(ResourceConflictError {
                        requested: requested_key.clone(),
                        held_by_task: lease.task_id,
                        held_lease: lease.lease_id,
                    });
                }
            }
        }

        Ok(())
    }

    /// Acquire locks atomically for a task across all requested resources.
    ///
    /// - Sorts requested keys into a deterministic global order (D-01) to prevent lock-order inversion.
    /// - Merges duplicate requests (promoting Shared to Exclusive).
    /// - Checks all active leases; fails atomically (all-or-none) if any conflict is detected.
    /// - Returns an RAII `ResourceLeaseGuard` that will release locks when dropped.
    pub fn acquire(
        &self,
        task_id: TaskId,
        mission_id: MissionId,
        requests: Vec<(ResourceKey, LockMode)>,
    ) -> Result<ResourceLeaseGuard, ResourceConflictError> {
        self.acquire_internal(task_id, mission_id, requests, true)
    }

    fn acquire_internal(
        &self,
        task_id: TaskId,
        mission_id: MissionId,
        requests: Vec<(ResourceKey, LockMode)>,
        spawn_bg: bool,
    ) -> Result<ResourceLeaseGuard, ResourceConflictError> {
        if requests.is_empty() {
            return Ok(ResourceLeaseGuard::new(
                task_id,
                Vec::new(),
                Some(Arc::new(self.clone())),
            ));
        }

        // 1 & 2. Sort and deduplicate in deterministic global order
        let mut merged: BTreeMap<ResourceKey, LockMode> = BTreeMap::new();
        for (key, mode) in requests {
            merged
                .entry(key)
                .and_modify(|existing| {
                    if mode == LockMode::Exclusive {
                        *existing = LockMode::Exclusive;
                    }
                })
                .or_insert(mode);
        }
        let deduped: Vec<(ResourceKey, LockMode)> = merged.into_iter().collect();

        // 3. Conflict check under lock (all-or-none)
        let mut inner = self.inner.lock().unwrap();

        for (key, mode) in &deduped {
            for lease in inner.leases.values() {
                if lease.task_id != task_id && key.conflicts_with(*mode, &lease.key, lease.mode) {
                    return Err(ResourceConflictError {
                        requested: key.clone(),
                        held_by_task: lease.task_id,
                        held_lease: lease.lease_id,
                    });
                }
            }
        }

        // 4. Allocate leases
        let now = Utc::now();
        let mut created_leases = Vec::with_capacity(deduped.len());
        let mut lease_ids = Vec::with_capacity(deduped.len());

        for (key, mode) in deduped {
            let lease_id = LeaseId::new();
            let lease = ActiveLease {
                lease_id,
                task_id,
                mission_id,
                key,
                mode,
                acquired_at: now,
                owner_generation: self.owner_generation,
            };
            inner.leases.insert(lease_id, lease.clone());
            inner.task_leases.entry(task_id).or_default().push(lease_id);
            lease_ids.push(lease_id);
            created_leases.push(lease);
        }

        drop(inner);

        // 5. Asynchronous persistence to SQLite if requested and pool is present
        if spawn_bg && let Some(ref pool) = self.pool {
            let pool = pool.clone();
            let leases_to_persist = created_leases;
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = insert_leases_sqlite(&pool, &leases_to_persist).await;
                });
            }
        }

        // 6. Return RAII guard
        Ok(ResourceLeaseGuard::new(
            task_id,
            lease_ids,
            Some(Arc::new(self.clone())),
        ))
    }

    /// Acquire locks and synchronously await SQLite persistence if database pool is present.
    pub async fn acquire_persisted(
        &self,
        task_id: TaskId,
        mission_id: MissionId,
        requests: Vec<(ResourceKey, LockMode)>,
    ) -> Result<ResourceLeaseGuard, ResourceConflictError> {
        let guard = self.acquire_internal(task_id, mission_id, requests, false)?;
        if let Some(ref pool) = self.pool {
            let active = self.get_task_leases(task_id);
            let _ = insert_leases_sqlite(pool, &active)
                .await
                .map_err(|e| ResourceConflictError {
                    requested: ResourceKey::workspace(e.to_string(), PathScope::Exact),
                    held_by_task: task_id,
                    held_lease: LeaseId::new(),
                });
        }
        Ok(guard)
    }

    /// Release all leases held by a task from in-memory maps and mark released in SQLite.
    pub fn release_task_leases(&self, task_id: TaskId) -> Vec<ActiveLease> {
        self.release_task_leases_internal(task_id, true)
    }

    fn release_task_leases_internal(&self, task_id: TaskId, spawn_bg: bool) -> Vec<ActiveLease> {
        let mut inner = self.inner.lock().unwrap();
        let lease_ids = inner.task_leases.remove(&task_id).unwrap_or_default();
        let mut released = Vec::with_capacity(lease_ids.len());
        for lid in lease_ids {
            if let Some(lease) = inner.leases.remove(&lid) {
                released.push(lease);
            }
        }
        drop(inner);

        if spawn_bg && let Some(ref pool) = self.pool {
            let pool = pool.clone();
            let now = Utc::now();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = record_release_sqlite(&pool, task_id, now).await;
                });
            }
        }

        released
    }

    /// Release all leases held by a task and synchronously await SQLite update.
    pub async fn release_task_leases_persisted(&self, task_id: TaskId) -> Vec<ActiveLease> {
        let released = self.release_task_leases_internal(task_id, false);
        if let Some(ref pool) = self.pool {
            let _ = record_release_sqlite(pool, task_id, Utc::now()).await;
        }
        released
    }

    /// Revoke all leases held by a task (e.g. on worker loss or crash recovery per D-03).
    pub fn revoke_task_leases(&self, task_id: TaskId, _reason: &str) -> Vec<ActiveLease> {
        self.release_task_leases(task_id)
    }
}

impl LeaseReleaser for ResourceManager {
    fn release_leases(&self, task_id: TaskId, _lease_ids: &[LeaseId]) {
        self.release_task_leases(task_id);
    }
}

async fn insert_leases_sqlite(pool: &SqlitePool, leases: &[ActiveLease]) -> Result<(), M31AError> {
    for lease in leases {
        let lock_mode_str = match lease.mode {
            LockMode::Shared => "Shared",
            LockMode::Exclusive => "Exclusive",
        };
        let scope_str = match lease.key.scope {
            PathScope::Exact => "Exact",
            PathScope::Subtree => "Subtree",
        };
        let acquired_at_str = lease.acquired_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO resource_leases (
                id, task_id, mission_id, resource_key, lock_mode, scope, acquired_at, owner_generation, released_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(lease.lease_id.as_bytes().as_slice())
        .bind(lease.task_id.as_bytes().as_slice())
        .bind(lease.mission_id.as_bytes().as_slice())
        .bind(&lease.key.canonical_id)
        .bind(lock_mode_str)
        .bind(scope_str)
        .bind(&acquired_at_str)
        .bind(lease.owner_generation as i64)
        .execute(pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;
    }
    Ok(())
}

async fn record_release_sqlite(
    pool: &SqlitePool,
    task_id: TaskId,
    released_at: DateTime<Utc>,
) -> Result<(), M31AError> {
    let released_at_str = released_at.to_rfc3339();

    sqlx::query(
        r#"
        UPDATE resource_leases
        SET released_at = ?
        WHERE task_id = ? AND released_at IS NULL
        "#,
    )
    .bind(&released_at_str)
    .bind(task_id.as_bytes().as_slice())
    .execute(pool)
    .await
    .map_err(|e| M31AError::persistence(e.to_string()))?;

    Ok(())
}
