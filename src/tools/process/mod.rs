//! Process and background job model-facing tools (TL-02, D-07, D-13).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::jobs::{JobDescriptor, JobOutputChunk, JobService, JobStatusInfo};
use crate::capability::traits::process::{ProcessOutput, ProcessService};
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn get_process(ctx: &ToolExecutionContext) -> Result<Arc<dyn ProcessService>, ToolError> {
    ctx.capability_registry
        .process()
        .ok_or_else(|| ToolError::capability_unavailable("process", None))
}

fn get_jobs(ctx: &ToolExecutionContext) -> Result<Arc<dyn JobService>, ToolError> {
    ctx.capability_registry
        .jobs()
        .ok_or_else(|| ToolError::capability_unavailable("jobs", None))
}

// ---------------------------------------------------------------------------
// 11. run_command
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunCommandInput {
    pub command: String,
    #[serde(
        default,
        deserialize_with = "crate::tools::qa::deserialize_flexible_string_vec"
    )]
    pub args: Option<Vec<String>>,
    pub timeout_secs: Option<u64>,
}

pub struct RunCommandTool;

#[async_trait]
impl TypedTool for RunCommandTool {
    type Input = RunCommandInput;
    type Output = ProcessOutput;

    fn id(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Run an isolated foreground process command with bounded execution time."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Process]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ProcessExecution
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(60, 5 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let process = get_process(ctx)?;
        let args = input.args.unwrap_or_default();
        // Single authority: per-call default mirrors the canonical tool
        // timeout; the enforced bound is `ResourceLimits` (clamped to the
        // immutable ceiling in `tools::definition`).
        let timeout_secs = input
            .timeout_secs
            .unwrap_or(crate::config::canonical::DEFAULT_TOOL_TIMEOUT_SECS);

        let output = process
            .spawn_command(
                &input.command,
                &args,
                Some(&ctx.workspace_root),
                timeout_secs,
            )
            .await?;

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// 12. start_job
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartJobInput {
    pub command: String,
    pub args: Option<Vec<String>>,
}

pub struct StartJobTool;

#[async_trait]
impl TypedTool for StartJobTool {
    type Input = StartJobInput;
    type Output = JobDescriptor;

    fn id(&self) -> &str {
        "start_job"
    }

    fn description(&self) -> &str {
        "Launch an asynchronous supervised background job."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Jobs]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ProcessExecution
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let jobs = get_jobs(ctx)?;
        let args = input.args.unwrap_or_default();
        // Real execution identity propagates into the durable job record
        // (never fresh identifiers). A tool context without mission/task
        // identity cannot start an attributable background job: fail closed.
        let mission_id = ctx.mission_id.ok_or_else(|| {
            ToolError::permission_denied(
                "JOB_AUTHORIZATION_UNBOUND",
                Some("start_job requires a bound mission identity".to_string()),
            )
        })?;
        let task_id = ctx.task_id.ok_or_else(|| {
            ToolError::permission_denied(
                "JOB_AUTHORIZATION_UNBOUND",
                Some("start_job requires a bound task identity".to_string()),
            )
        })?;
        let agent_id = ctx.agent_id.ok_or_else(|| {
            ToolError::permission_denied(
                "JOB_AUTHORIZATION_UNBOUND",
                Some("start_job requires a bound agent identity".to_string()),
            )
        })?;
        // Effective job limits derive from this tool's declared resource
        // budget (the authoritative per-tool ceiling), persisted with the
        // job record for enforcement and restart reconciliation.
        let tool_limits = self.resource_limits();
        let job_limits = crate::sandbox::ResourceLimits::new(
            tool_limits.timeout_secs * 1000,
            tool_limits.max_output_bytes,
        );
        let descriptor = jobs
            .start_job_scoped(
                mission_id,
                task_id,
                agent_id,
                job_limits,
                &input.command,
                &args,
            )
            .await?;
        Ok(descriptor)
    }
}

// ---------------------------------------------------------------------------
// 13. job_status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobStatusInput {
    pub job_id: String,
}

pub struct JobStatusTool;

#[async_trait]
impl TypedTool for JobStatusTool {
    type Input = JobStatusInput;
    type Output = JobStatusInfo;

    fn id(&self) -> &str {
        "job_status"
    }

    fn description(&self) -> &str {
        "Query the current lifecycle state and exit status of a background job."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Jobs]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let jobs = get_jobs(ctx)?;
        let status = jobs.get_status(&input.job_id).await?;
        Ok(status)
    }
}

// ---------------------------------------------------------------------------
// 14. job_output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobOutputInput {
    pub job_id: String,
    pub offset: Option<u64>,
    pub limit: Option<usize>,
}

pub struct JobOutputTool;

#[async_trait]
impl TypedTool for JobOutputTool {
    type Input = JobOutputInput;
    type Output = JobOutputChunk;

    fn id(&self) -> &str {
        "job_output"
    }

    fn description(&self) -> &str {
        "Retrieve bounded output chunks from a running or completed background job."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Jobs]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let jobs = get_jobs(ctx)?;
        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(64 * 1024);
        let chunk = jobs.get_output(&input.job_id, offset, limit).await?;
        Ok(chunk)
    }
}

// ---------------------------------------------------------------------------
// 15. job_stop
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobStopInput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobStopOutput {
    pub job_id: String,
    pub stopped: bool,
}

pub struct JobStopTool;

#[async_trait]
impl TypedTool for JobStopTool {
    type Input = JobStopInput;
    type Output = JobStopOutput;

    fn id(&self) -> &str {
        "job_stop"
    }

    fn description(&self) -> &str {
        "Terminate and clean up a running background job process tree."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Jobs]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let jobs = get_jobs(ctx)?;
        jobs.stop_job(&input.job_id).await?;
        Ok(JobStopOutput {
            job_id: input.job_id,
            stopped: true,
        })
    }
}
