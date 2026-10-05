//! Background jobs supervision service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;

pub use crate::process::{JobDescriptor, JobOutputChunk, JobStatusInfo};

/// Asynchronous service seam for background job lifecycle and streams.
#[async_trait]
pub trait JobService: Send + Sync + 'static {
    /// Launch a supervised asynchronous background job.
    ///
    /// COMPATIBILITY ONLY: carries no execution identity, so providers must
    /// treat jobs started this way as unattributed. Production paths MUST use
    /// [`start_job_scoped`](Self::start_job_scoped) with the real
    /// mission/task/agent identity and effective resource limits.
    async fn start_job(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<JobDescriptor, CapabilityError>;

    /// Launch a background job bound to the real execution context.
    ///
    /// Canonical production entry: identity propagates (never fresh
    /// identifiers) and the effective resource limits persist with the job
    /// record. The default body delegates to [`start_job`](Self::start_job)
    /// for providers that have not migrated; governed providers override it.
    async fn start_job_scoped(
        &self,
        _mission_id: crate::ids::MissionId,
        _task_id: crate::ids::TaskId,
        _agent_id: crate::ids::AgentId,
        _resource_limits: crate::sandbox::ResourceLimits,
        command: &str,
        args: &[String],
    ) -> Result<JobDescriptor, CapabilityError> {
        self.start_job(command, args).await
    }

    /// Check lifecycle status of a background job.
    async fn get_status(&self, job_id: &str) -> Result<JobStatusInfo, CapabilityError>;

    /// Fetch buffered stdout/stderr output slice starting from offset.
    async fn get_output(
        &self,
        job_id: &str,
        offset: u64,
        limit: usize,
    ) -> Result<JobOutputChunk, CapabilityError>;

    /// Stop and terminate a running job process tree.
    async fn stop_job(&self, job_id: &str) -> Result<(), CapabilityError>;
}
