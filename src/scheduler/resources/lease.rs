use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

use super::key::{LockMode, ResourceKey};
use crate::ids::{MissionId, TaskId};

/// Strongly typed identifier for a resource lease (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeaseId(Uuid);

impl LeaseId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Serialize for LeaseId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for LeaseId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(LeaseId(uuid))
    }
}

impl fmt::Display for LeaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for LeaseId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<LeaseId> for Uuid {
    fn from(id: LeaseId) -> Uuid {
        id.0
    }
}

impl FromStr for LeaseId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(LeaseId)
    }
}

impl Default for LeaseId {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata describing an actively held resource lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLease {
    pub lease_id: LeaseId,
    pub task_id: TaskId,
    pub mission_id: MissionId,
    pub key: ResourceKey,
    pub mode: LockMode,
    pub acquired_at: DateTime<Utc>,
    pub owner_generation: u64,
}

/// Callback interface for dropping resource leases.
pub trait LeaseReleaser: Send + Sync {
    fn release_leases(&self, task_id: TaskId, lease_ids: &[LeaseId]);
}

/// RAII guard providing scoped lifetime management for acquired resource leases.
///
/// When dropped, automatically invokes lease release on the owning manager unless
/// explicitly disarmed via `disarm()`.
pub struct ResourceLeaseGuard {
    task_id: TaskId,
    lease_ids: Vec<LeaseId>,
    releaser: Option<Arc<dyn LeaseReleaser>>,
}

impl ResourceLeaseGuard {
    pub fn new(
        task_id: TaskId,
        lease_ids: Vec<LeaseId>,
        releaser: Option<Arc<dyn LeaseReleaser>>,
    ) -> Self {
        Self {
            task_id,
            lease_ids,
            releaser,
        }
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub fn lease_ids(&self) -> &[LeaseId] {
        &self.lease_ids
    }

    /// Disarm the RAII guard, preventing drop cleanup.
    ///
    /// Returns the vector of held `LeaseId`s for manual lifecycle management
    /// (e.g. migration, crash recovery preservation).
    pub fn disarm(mut self) -> Vec<LeaseId> {
        self.releaser = None;
        std::mem::take(&mut self.lease_ids)
    }
}

impl fmt::Debug for ResourceLeaseGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceLeaseGuard")
            .field("task_id", &self.task_id)
            .field("lease_ids", &self.lease_ids)
            .finish()
    }
}

impl Drop for ResourceLeaseGuard {
    fn drop(&mut self) {
        if let Some(releaser) = self.releaser.take() {
            releaser.release_leases(self.task_id, &self.lease_ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestReleaser {
        released: AtomicBool,
    }

    impl LeaseReleaser for TestReleaser {
        fn release_leases(&self, _task_id: TaskId, _lease_ids: &[LeaseId]) {
            self.released.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_lease_id_serde_roundtrip() {
        let id = LeaseId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: LeaseId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_lease_id_from_str() {
        let id = LeaseId::new();
        let s = id.to_string();
        let parsed: LeaseId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_resource_lease_guard_drop_releases() {
        let releaser = Arc::new(TestReleaser {
            released: AtomicBool::new(false),
        });
        let task_id = TaskId::new();
        let lease_id = LeaseId::new();

        {
            let _guard = ResourceLeaseGuard::new(task_id, vec![lease_id], Some(releaser.clone()));
            assert!(!releaser.released.load(Ordering::SeqCst));
        }

        assert!(releaser.released.load(Ordering::SeqCst));
    }

    #[test]
    fn test_resource_lease_guard_disarm_prevents_release() {
        let releaser = Arc::new(TestReleaser {
            released: AtomicBool::new(false),
        });
        let task_id = TaskId::new();
        let lease_id = LeaseId::new();

        {
            let guard = ResourceLeaseGuard::new(task_id, vec![lease_id], Some(releaser.clone()));
            let disarmed_ids = guard.disarm();
            assert_eq!(disarmed_ids, vec![lease_id]);
        }

        assert!(!releaser.released.load(Ordering::SeqCst));
    }
}
