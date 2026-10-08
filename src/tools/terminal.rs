//! Terminal session model-facing tools (CTL-01, CTL-02, TL-02).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::terminal::{
    TerminalService, TerminalSessionConfig, TerminalSessionStatus,
};
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn get_terminal(ctx: &ToolExecutionContext) -> Result<Arc<dyn TerminalService>, ToolError> {
    ctx.capability_registry
        .terminal()
        .ok_or_else(|| ToolError::capability_unavailable("terminal", None))
}

// ---------------------------------------------------------------------------
// 1. terminal_start_session
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalStartSessionInput {
    pub session_id: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalStartSessionOutput {
    pub session_id: String,
    pub started: bool,
}

pub struct TerminalStartSessionTool;

#[async_trait]
impl TypedTool for TerminalStartSessionTool {
    type Input = TerminalStartSessionInput;
    type Output = TerminalStartSessionOutput;

    fn id(&self) -> &str {
        "terminal_start_session"
    }

    fn description(&self) -> &str {
        "Start an interactive pseudo-terminal session with specified command, environment, and dimensions."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
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
        let terminal = get_terminal(ctx)?;
        let mut config = TerminalSessionConfig::default();
        if let Some(cmd) = input.command {
            config.command = cmd;
        }
        if let Some(args) = input.args {
            config.args = args;
        }
        if let Some(cwd) = input.cwd {
            config.cwd = Some(PathBuf::from(cwd));
        } else {
            config.cwd = Some(ctx.workspace_root.clone());
        }
        if let Some(env) = input.env {
            config.env = env;
        }
        if let Some(cols) = input.cols {
            config.cols = cols;
        }
        if let Some(rows) = input.rows {
            config.rows = rows;
        }
        config.owner_agent_id = ctx.agent_id;
        config.owner_mission_id = ctx.mission_id;

        terminal.start_session(&input.session_id, config).await?;

        Ok(TerminalStartSessionOutput {
            session_id: input.session_id,
            started: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 2. terminal_write_input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalWriteInputInput {
    pub session_id: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalWriteInputOutput {
    pub session_id: String,
    pub bytes_written: usize,
}

pub struct TerminalWriteInputTool;

#[async_trait]
impl TypedTool for TerminalWriteInputTool {
    type Input = TerminalWriteInputInput;
    type Output = TerminalWriteInputOutput;

    fn id(&self) -> &str {
        "terminal_write_input"
    }

    fn description(&self) -> &str {
        "Write input text/bytes to an active interactive terminal session stream."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ProcessExecution
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let terminal = get_terminal(ctx)?;
        let bytes = input.input.as_bytes();
        terminal.write_input(&input.session_id, bytes).await?;

        Ok(TerminalWriteInputOutput {
            session_id: input.session_id,
            bytes_written: bytes.len(),
        })
    }
}

// ---------------------------------------------------------------------------
// 3. terminal_read_stream
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalReadStreamInput {
    pub session_id: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalReadStreamOutput {
    pub session_id: String,
    pub output: String,
    pub bytes_read: usize,
}

pub struct TerminalReadStreamTool;

#[async_trait]
impl TypedTool for TerminalReadStreamTool {
    type Input = TerminalReadStreamInput;
    type Output = TerminalReadStreamOutput;

    fn id(&self) -> &str {
        "terminal_read_stream"
    }

    fn description(&self) -> &str {
        "Read available output bytes from an interactive terminal session stream up to timeout."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let terminal = get_terminal(ctx)?;
        let timeout_ms = input.timeout_ms.unwrap_or(200);
        let raw = terminal.read_stream(&input.session_id, timeout_ms).await?;
        let output = String::from_utf8_lossy(&raw).to_string();

        Ok(TerminalReadStreamOutput {
            session_id: input.session_id,
            bytes_read: raw.len(),
            output,
        })
    }
}

// ---------------------------------------------------------------------------
// 4. terminal_resize
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalResizeInput {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalResizeOutput {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
    pub resized: bool,
}

pub struct TerminalResizeTool;

#[async_trait]
impl TypedTool for TerminalResizeTool {
    type Input = TerminalResizeInput;
    type Output = TerminalResizeOutput;

    fn id(&self) -> &str {
        "terminal_resize"
    }

    fn description(&self) -> &str {
        "Resize the window dimensions (columns and rows) of an interactive terminal session."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let terminal = get_terminal(ctx)?;
        terminal
            .resize(&input.session_id, input.cols, input.rows)
            .await?;

        Ok(TerminalResizeOutput {
            session_id: input.session_id,
            cols: input.cols,
            rows: input.rows,
            resized: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 5. terminal_session_status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalSessionStatusInput {
    pub session_id: String,
}

pub struct TerminalSessionStatusTool;

#[async_trait]
impl TypedTool for TerminalSessionStatusTool {
    type Input = TerminalSessionStatusInput;
    type Output = TerminalSessionStatus;

    fn id(&self) -> &str {
        "terminal_session_status"
    }

    fn description(&self) -> &str {
        "Query the current liveness, exit code, and dimensions of an interactive terminal session."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
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
        let terminal = get_terminal(ctx)?;
        let status = terminal.session_status(&input.session_id).await?;
        Ok(status)
    }
}

// ---------------------------------------------------------------------------
// 6. terminal_terminate_session
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalTerminateSessionInput {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalTerminateSessionOutput {
    pub session_id: String,
    pub terminated: bool,
}

pub struct TerminalTerminateSessionTool;

#[async_trait]
impl TypedTool for TerminalTerminateSessionTool {
    type Input = TerminalTerminateSessionInput;
    type Output = TerminalTerminateSessionOutput;

    fn id(&self) -> &str {
        "terminal_terminate_session"
    }

    fn description(&self) -> &str {
        "Terminate and clean up an active interactive terminal session."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Terminal]
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
        let terminal = get_terminal(ctx)?;
        terminal.terminate_session(&input.session_id).await?;

        Ok(TerminalTerminateSessionOutput {
            session_id: input.session_id,
            terminated: true,
        })
    }
}
