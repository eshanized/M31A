//! Process and background job core types (CTL-01, CTL-02).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Result of executing a process command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Metadata descriptor of a started background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobDescriptor {
    pub job_id: String,
    pub command: String,
    pub pid: Option<u32>,
    pub started_at_ms: u64,
}

/// Status snapshot of a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobStatusInfo {
    pub job_id: String,
    pub state: String,
    pub exit_code: Option<i32>,
    pub running_ms: u64,
}

/// Output chunk returned from querying background job output streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobOutputChunk {
    pub job_id: String,
    pub stdout: String,
    pub stderr: String,
    pub next_offset: u64,
    pub is_eof: bool,
}
