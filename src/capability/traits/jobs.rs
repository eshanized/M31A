//! Background jobs supervision service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;

pub use crate::process::{JobDescriptor, JobOutputChunk, JobStatusInfo};

/// Asynchronous service seam for background job lifecycle and streams.
#[async_trait]
pub trait JobService: Send + Sync + 'static {
    /// Launch a supervised asynchronous background job.
    async fn start_job(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<JobDescriptor, CapabilityError>;

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
