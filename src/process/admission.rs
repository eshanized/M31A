//! Dedicated background job admission controller and concurrency permits (JOB-01, D-15).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::ids::MissionId;
use crate::process::job::JobError;

pub const DEFAULT_MAX_GLOBAL_CONCURRENT_JOBS: usize = 16;
pub const DEFAULT_MAX_PER_MISSION_CONCURRENT_JOBS: usize = 4;

/// RAII job execution permit guard.
///
/// Dropping this permit immediately returns capacity to both the global
/// pool and the mission-specific pool on every exit, cancellation, or panic.
pub struct JobPermit {
    _global: OwnedSemaphorePermit,
    _mission: OwnedSemaphorePermit,
}

impl std::fmt::Debug for JobPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobPermit").finish()
    }
}

/// Dedicated job admission controller enforcing hierarchical concurrency bounds (D-15).
#[derive(Clone)]
pub struct JobAdmissionController {
    global_semaphore: Arc<Semaphore>,
    mission_semaphores: Arc<Mutex<HashMap<MissionId, Arc<Semaphore>>>>,
    max_global: usize,
    max_per_mission: usize,
}

impl Default for JobAdmissionController {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_GLOBAL_CONCURRENT_JOBS,
            DEFAULT_MAX_PER_MISSION_CONCURRENT_JOBS,
        )
    }
}

impl JobAdmissionController {
    /// Create an admission controller with specified global and per-mission limits.
    pub fn new(max_global: usize, max_per_mission: usize) -> Self {
        Self {
            global_semaphore: Arc::new(Semaphore::new(max_global)),
            mission_semaphores: Arc::new(Mutex::new(HashMap::new())),
            max_global,
            max_per_mission,
        }
    }

    /// Asynchronously acquire a hierarchical job execution permit bounded by timeout.
    pub async fn acquire_permit(
        &self,
        mission_id: MissionId,
        timeout: Duration,
    ) -> Result<JobPermit, JobError> {
        let start = Instant::now();

        // 1. Acquire global capacity permit
        let global_permit = match tokio::time::timeout(
            timeout,
            self.global_semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                return Err(JobError::StartFailed(
                    "Global job admission semaphore was closed".into(),
                ));
            }
            Err(_) => {
                return Err(JobError::CapacityExceeded(format!(
                    "Global job capacity ({}) full, timed out after {:?}",
                    self.max_global, timeout
                )));
            }
        };

        // 2. Obtain or initialize mission-specific semaphore
        let elapsed = start.elapsed();
        let remaining_timeout = timeout.saturating_sub(elapsed);
        if remaining_timeout.is_zero() {
            return Err(JobError::CapacityExceeded(format!(
                "Timed out before acquiring mission capacity for mission '{}'",
                mission_id
            )));
        }

        let mission_sem = {
            let mut map = self.mission_semaphores.lock().await;
            map.entry(mission_id)
                .or_insert_with(|| Arc::new(Semaphore::new(self.max_per_mission)))
                .clone()
        };

        // 3. Acquire mission-specific permit
        let mission_permit =
            match tokio::time::timeout(remaining_timeout, mission_sem.acquire_owned()).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    return Err(JobError::StartFailed(
                        "Mission job admission semaphore was closed".into(),
                    ));
                }
                Err(_) => {
                    return Err(JobError::CapacityExceeded(format!(
                        "Mission '{}' job capacity ({}) full, timed out after {:?}",
                        mission_id, self.max_per_mission, timeout
                    )));
                }
            };

        Ok(JobPermit {
            _global: global_permit,
            _mission: mission_permit,
        })
    }

    /// Query available global and mission capacity.
    pub async fn available_permits(&self, mission_id: MissionId) -> (usize, usize) {
        let global_avail = self.global_semaphore.available_permits();
        let mut map = self.mission_semaphores.lock().await;
        let mission_sem = map
            .entry(mission_id)
            .or_insert_with(|| Arc::new(Semaphore::new(self.max_per_mission)));
        let mission_avail = mission_sem.available_permits();
        (global_avail, mission_avail)
    }
}
