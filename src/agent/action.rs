//! Canonical Action/Outcome Protocol (Issue 21, ACT-01..04).
//!
//! "The model proposes. The runtime decides."
//!
//! Provides the strongly typed runtime protocol bridging:
//! - Model proposals
//! - AgentEngine / WorkerRunner
//! - Task scheduler
//! - Workflow execution
//! - Tool governance and sandbox
//! - Verification hierarchy
//! - Telemetry and TUI projection

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::agent::supervisor::AgentOutcome;
use crate::ids::{AgentId, TaskId};
use crate::state_machine::agent::AgentRole;

/// Granular user option presented during an interactive decision point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionUserOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Diagnostic summary entry associated with an action observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticRecord {
    pub tool: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
    pub suggested_fix: Option<String>,
}

/// Canonical typed runtime actions proposed by agents or interactive commands (Issue 21).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action_type", content = "payload", rename_all = "snake_case")]
pub enum AgentAction {
    /// Execute a governed tool through the 11-stage pipeline.
    CallTool {
        tool_name: String,
        parameters: serde_json::Value,
    },
    /// Write or update a file within the workspace.
    WriteChange {
        path: PathBuf,
        content: String,
        description: Option<String>,
    },
    /// Run verification tier checks against workspace state.
    RunVerification {
        tier: Option<u8>,
        target: Option<String>,
    },
    /// Ask operator for semantic clarification with options.
    AskUser {
        question: String,
        options: Vec<ActionUserOption>,
    },
    /// Request operator policy approval for a privileged operation.
    RequestApproval {
        tool_name: String,
        parameters: serde_json::Value,
        risk: Option<String>,
        reason: String,
    },
    /// Introduce a new formed task into the active task graph.
    CreateTask {
        title: String,
        description: String,
        role: Option<AgentRole>,
        dependencies: Vec<TaskId>,
    },
    /// Invalidate/supersede a stale task with causal reason.
    SupersedeTask { task_id: TaskId, reason: String },
    /// Request adaptive replanning of remaining work.
    Replan {
        reason: String,
        tasks_to_supersede: Vec<TaskId>,
        new_tasks: Vec<String>,
    },
    /// Delegate work to a child agent under role boundary.
    Delegate {
        target_role: AgentRole,
        task_id: Option<TaskId>,
        reason: String,
        inputs: Vec<String>,
    },
    /// Pause active execution.
    Pause { reason: String },
    /// Resume paused execution.
    Resume { reason: Option<String> },
    /// Cancel active in-flight work.
    Cancel { reason: String },
    /// Declare task completion with verified evidence.
    Complete { summary: String },
    /// Declare task failure with diagnostic error classification.
    Fail { error: String },
}

impl AgentAction {
    /// canonical action kind name for telemetry, auditing, and events.
    pub fn name(&self) -> &'static str {
        match self {
            Self::CallTool { .. } => "call_tool",
            Self::WriteChange { .. } => "write_change",
            Self::RunVerification { .. } => "run_verification",
            Self::AskUser { .. } => "ask_user",
            Self::RequestApproval { .. } => "request_approval",
            Self::CreateTask { .. } => "create_task",
            Self::SupersedeTask { .. } => "supersede_task",
            Self::Replan { .. } => "replan",
            Self::Delegate { .. } => "delegate",
            Self::Pause { .. } => "pause",
            Self::Resume { .. } => "resume",
            Self::Cancel { .. } => "cancel",
            Self::Complete { .. } => "complete",
            Self::Fail { .. } => "fail",
        }
    }

    /// canonical translation from a raw model tool call to strongly typed agent action.
    pub fn from_tool_call(call: &crate::model::types::ModelToolCall) -> Self {
        match call.name.as_str() {
            "adapt_strategy" => {
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("model requested strategy adaptation")
                    .to_string();
                let tasks_to_supersede = call
                    .arguments
                    .get("tasks_to_supersede")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .filter_map(|s| s.parse::<uuid::Uuid>().ok())
                            .map(TaskId::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let new_tasks = call
                    .arguments
                    .get("new_tasks")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                Self::Replan {
                    reason,
                    tasks_to_supersede,
                    new_tasks,
                }
            }
            "write_file" | "edit_file" | "apply_patch" => {
                if let (Some(path_str), Some(content_str)) = (
                    call.arguments.get("path").and_then(|v| v.as_str()),
                    call.arguments.get("content").and_then(|v| v.as_str()),
                ) {
                    Self::WriteChange {
                        path: PathBuf::from(path_str),
                        content: content_str.to_string(),
                        description: call
                            .arguments
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string),
                    }
                } else {
                    Self::CallTool {
                        tool_name: call.name.clone(),
                        parameters: call.arguments.clone(),
                    }
                }
            }
            "run_tests" | "run_verification" => {
                let tier = call
                    .arguments
                    .get("tier")
                    .and_then(|v| v.as_u64())
                    .map(|t| t as u8);
                let target = call
                    .arguments
                    .get("target")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                Self::RunVerification { tier, target }
            }
            "ask_user" => {
                let question = call
                    .arguments
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("clarification needed")
                    .to_string();
                let options = call
                    .arguments
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| {
                                serde_json::from_value::<ActionUserOption>(o.clone()).ok()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Self::AskUser { question, options }
            }
            "request_approval" => {
                let tool_name = call
                    .arguments
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let parameters = call
                    .arguments
                    .get("parameters")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let risk = call
                    .arguments
                    .get("risk")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("approval requested")
                    .to_string();
                Self::RequestApproval {
                    tool_name,
                    parameters,
                    risk,
                    reason,
                }
            }
            "create_task" => {
                let title = call
                    .arguments
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("untitled")
                    .to_string();
                let description = call
                    .arguments
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let role = call
                    .arguments
                    .get("role")
                    .and_then(|v| v.as_str())
                    .map(AgentRole::new);
                let dependencies = call
                    .arguments
                    .get("dependencies")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .filter_map(|s| s.parse::<uuid::Uuid>().ok())
                            .map(TaskId::from)
                            .collect()
                    })
                    .unwrap_or_default();
                Self::CreateTask {
                    title,
                    description,
                    role,
                    dependencies,
                }
            }
            "supersede_task" => {
                let task_id = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<uuid::Uuid>().ok())
                    .map(TaskId::from)
                    .unwrap_or_default();
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("superseded")
                    .to_string();
                Self::SupersedeTask { task_id, reason }
            }
            _ => Self::CallTool {
                tool_name: call.name.clone(),
                parameters: call.arguments.clone(),
            },
        }
    }

    /// canonical translation from a model proposal to one or more strongly typed agent actions.
    pub fn from_model_proposal(proposal: &crate::model::types::ModelProposal) -> Vec<Self> {
        match proposal {
            crate::model::types::ModelProposal::ToolCalls { calls } => {
                calls.iter().map(Self::from_tool_call).collect()
            }
            crate::model::types::ModelProposal::AskUser { question, options } => {
                vec![Self::AskUser {
                    question: question.clone(),
                    options: options
                        .iter()
                        .map(|o| ActionUserOption {
                            id: o.id.clone(),
                            label: o.label.clone(),
                            description: o.description.clone(),
                        })
                        .collect(),
                }]
            }
            crate::model::types::ModelProposal::Handoff {
                target_role,
                reason,
            } => {
                vec![Self::Delegate {
                    target_role: AgentRole::new(target_role.clone()),
                    task_id: None,
                    reason: reason.clone(),
                    inputs: Vec::new(),
                }]
            }
            crate::model::types::ModelProposal::Complete { summary, .. } => {
                vec![Self::Complete {
                    summary: summary.clone(),
                }]
            }
            crate::model::types::ModelProposal::AssistantText { .. } => Vec::new(),
        }
    }
}

/// Canonical typed runtime observations generated by authorities after action execution (Issue 21).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "observation_type",
    content = "payload",
    rename_all = "snake_case"
)]
pub enum AgentObservation {
    /// Output produced by tool pipeline execution.
    ToolOutput {
        tool_name: String,
        call_id: String,
        output: String,
        success: bool,
        duration_ms: u64,
    },
    /// File written or updated with content-addressed hash.
    ChangeWritten {
        path: PathBuf,
        bytes_written: usize,
        content_hash: String,
    },
    /// Verification hierarchy evaluation outcome.
    VerificationResult {
        passed: bool,
        summary: String,
        tier: u8,
        diagnostics: Vec<DiagnosticRecord>,
    },
    /// User provided answer to an outstanding question.
    UserResponse { answer: String },
    /// Operator granted or denied an approval request.
    ApprovalResult {
        approved: bool,
        reason: Option<String>,
    },
    /// Task was registered in the task graph.
    TaskCreated { task_id: TaskId },
    /// Task was marked superseded.
    TaskSuperseded { task_id: TaskId },
    /// Replan was applied and persisted.
    ReplanCompleted {
        new_plan_revision: u32,
        superseded_count: usize,
        added_count: usize,
    },
    /// Subagent execution completed or failed.
    DelegationResult {
        child_agent_id: AgentId,
        outcome: Box<AgentOutcome>,
    },
    /// Execution was successfully paused.
    Paused,
    /// Execution was resumed.
    Resumed,
    /// Execution was cancelled.
    Cancelled { reason: String },
    /// Completion was validated and committed.
    Completed { summary: String },
    /// Failure was recorded and classified.
    Failed {
        error: String,
        classification: Option<String>,
    },
}

impl AgentObservation {
    /// check if the observation represents a successful outcome.
    pub fn is_success(&self) -> bool {
        match self {
            Self::ToolOutput { success, .. } => *success,
            Self::ChangeWritten { .. } => true,
            Self::VerificationResult { passed, .. } => *passed,
            Self::UserResponse { .. } => true,
            Self::ApprovalResult { approved, .. } => *approved,
            Self::TaskCreated { .. } => true,
            Self::TaskSuperseded { .. } => true,
            Self::ReplanCompleted { .. } => true,
            Self::DelegationResult { outcome, .. } => outcome.is_success(),
            Self::Paused => true,
            Self::Resumed => true,
            Self::Cancelled { .. } => false,
            Self::Completed { .. } => true,
            Self::Failed { .. } => false,
        }
    }

    /// extract textual output from observation for display and history.
    pub fn output_text(&self) -> String {
        match self {
            Self::ToolOutput { output, .. } => output.clone(),
            Self::ChangeWritten {
                path,
                bytes_written,
                ..
            } => {
                format!("wrote {} bytes to {}", bytes_written, path.display())
            }
            Self::VerificationResult { summary, .. } => summary.clone(),
            Self::UserResponse { answer } => answer.clone(),
            Self::ApprovalResult { approved, reason } => {
                format!(
                    "approval {}: {}",
                    if *approved { "granted" } else { "denied" },
                    reason.as_deref().unwrap_or("none")
                )
            }
            Self::TaskCreated { task_id } => format!("task created: {}", task_id),
            Self::TaskSuperseded { task_id } => format!("task superseded: {}", task_id),
            Self::ReplanCompleted {
                new_plan_revision,
                superseded_count,
                added_count,
            } => {
                format!(
                    "replan committed: revision {}, superseded {}, added {}",
                    new_plan_revision, superseded_count, added_count
                )
            }
            Self::DelegationResult { child_agent_id, .. } => {
                format!("delegation to child {} completed", child_agent_id)
            }
            Self::Paused => "paused".to_string(),
            Self::Resumed => "resumed".to_string(),
            Self::Cancelled { reason } => format!("cancelled: {}", reason),
            Self::Completed { summary } => summary.clone(),
            Self::Failed { error, .. } => error.clone(),
        }
    }

    /// extract error message if observation represents failure.
    pub fn error_text(&self) -> Option<String> {
        match self {
            Self::ToolOutput {
                success, output, ..
            } if !*success => Some(output.clone()),
            Self::VerificationResult {
                passed, summary, ..
            } if !*passed => Some(summary.clone()),
            Self::ApprovalResult { approved, reason } if !*approved => Some(format!(
                "approval denied: {}",
                reason.as_deref().unwrap_or("no reason provided")
            )),
            Self::Cancelled { reason } => Some(format!("cancelled: {}", reason)),
            Self::Failed { error, .. } => Some(error.clone()),
            _ => None,
        }
    }

    /// return operation duration in milliseconds if known.
    pub fn duration_ms(&self) -> u64 {
        match self {
            Self::ToolOutput { duration_ms, .. } => *duration_ms,
            _ => 0,
        }
    }
}

/// Durable execution record linking an action proposal to its authoritative outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionExecutionRecord {
    pub action_id: String,
    pub action: AgentAction,
    pub observation: AgentObservation,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub success: bool,
    pub policy_decision: Option<String>,
    pub correlation_id: Option<String>,
}
