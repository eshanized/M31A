//! Resource limits contract and pre-execution hook application (SND-01, D-08).

use serde::{Deserialize, Serialize};

/// Provider-independent resource execution limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Wall-clock execution timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum CPU execution time in seconds.
    pub cpu_time_secs: Option<u64>,
    /// Maximum virtual address space in bytes.
    pub memory_bytes: Option<u64>,
    /// Maximum number of simultaneously open file descriptors.
    pub max_open_files: Option<u64>,
    /// Maximum number of child processes.
    pub max_processes: Option<u64>,
    /// Maximum total output buffer size in bytes before promotion/truncation.
    pub max_output_bytes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            cpu_time_secs: Some(30),
            memory_bytes: Some(1024 * 1024 * 1024), // 1 GiB
            max_open_files: Some(256),
            max_processes: None,
            max_output_bytes: 10 * 1024 * 1024, // 10 MiB
        }
    }
}

impl ResourceLimits {
    /// Create new resource limits with specified timeout and output size.
    pub fn new(timeout_ms: u64, max_output_bytes: usize) -> Self {
        Self {
            timeout_ms,
            max_output_bytes,
            ..Default::default()
        }
    }
}

/// Apply resource limits prior to child process execution via pre-execution hook.
///
/// Enforcement lives in the platform boundary so higher layers never branch
/// on raw system calls. Invalid bounds are rejected as errors.
pub fn apply_pre_exec_limits(limits: &ResourceLimits) -> Result<(), std::io::Error> {
    let budget = crate::platform::resources::ResourceBudget {
        max_cpu_seconds: limits.cpu_time_secs,
        max_memory_bytes: limits.memory_bytes,
        max_processes: limits.max_processes,
        max_open_files: limits.max_open_files,
        max_output_bytes: Some(limits.max_output_bytes),
    };
    match crate::platform::resources::apply_pre_exec_limits(&budget) {
        Ok(crate::platform::resources::LimitOutcome::Unsupported) => Ok(()),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
