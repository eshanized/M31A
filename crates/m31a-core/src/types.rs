//! Core data types, chat message structures, tool calls, and token accounting models.
//!
//! Defines standard types used across all M31A crates (ChatMessage, Role, ToolCall, TokenUsage).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }
    }

    pub fn assistant(content: Option<String>, tool_calls: Option<Vec<ToolCall>>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
            reasoning_content: None,
        }
    }

    pub fn tool_response(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            reasoning_content: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub extra: HashMap<String, String>,
}

/// Lifecycle status of an agent cell or subagent process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRunStatus {
    Created,
    Starting,
    Running,
    AwaitingApproval,
    CancellationRequested,
    Cancelling,
    Cancelled,
    Completed,
    Failed(String),
    IntegrationPending,
    Integrated,
    IntegrationRejected(String),
    Closed,
}

impl AgentRunStatus {
    pub fn label(&self) -> String {
        match self {
            AgentRunStatus::Created => "created".to_string(),
            AgentRunStatus::Starting => "starting".to_string(),
            AgentRunStatus::Running => "running".to_string(),
            AgentRunStatus::AwaitingApproval => "awaiting_approval".to_string(),
            AgentRunStatus::CancellationRequested => "cancellation_requested".to_string(),
            AgentRunStatus::Cancelling => "cancelling".to_string(),
            AgentRunStatus::Completed => "completed".to_string(),
            AgentRunStatus::Failed(e) => format!("failed: {e}"),
            AgentRunStatus::Cancelled => "cancelled".to_string(),
            AgentRunStatus::Closed => "closed".to_string(),
            AgentRunStatus::IntegrationPending => "integration_pending".to_string(),
            AgentRunStatus::Integrated => "integrated".to_string(),
            AgentRunStatus::IntegrationRejected(e) => format!("integration_rejected: {e}"),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentRunStatus::Completed
                | AgentRunStatus::Failed(_)
                | AgentRunStatus::Cancelled
                | AgentRunStatus::Closed
                | AgentRunStatus::IntegrationPending
                | AgentRunStatus::Integrated
                | AgentRunStatus::IntegrationRejected(_)
        )
    }
}

/// Termination report for an agent tree rooted at `root_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeTerminationReport {
    pub root_id: String,
    pub terminated_agents: Vec<String>,
    pub timed_out_agents: Vec<String>,
    pub failed_agents: Vec<(String, String)>,
    pub is_complete: bool,
}

/// Truthful structured process outcome from command execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredProcessOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub total_bytes_drained: usize,
    pub truncated: bool,
}

/// Structured outcome of a tool execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: Option<String>,
    pub timed_out: bool,
    pub total_bytes_drained: usize,
}
