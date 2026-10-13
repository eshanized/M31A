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

/// Strongly typed error representing a failure to parse model tool call arguments.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionParseError {
    #[error("Missing required field '{field}' for action '{action}'")]
    MissingRequiredField {
        action: &'static str,
        field: &'static str,
    },
    #[error("Invalid parameter '{field}' for action '{action}': {reason}")]
    InvalidParameter {
        action: &'static str,
        field: &'static str,
        reason: String,
    },
}

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

    /// canonical translation from a raw model tool call to strongly typed agent action with strict validation.
    pub fn try_from_tool_call(
        call: &crate::model::types::ModelToolCall,
    ) -> Result<Self, ActionParseError> {
        match call.name.as_str() {
            "adapt_strategy" => {
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "adapt_strategy",
                        field: "reason",
                    })?;
                if reason.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "adapt_strategy",
                        field: "reason",
                        reason: "reason cannot be empty".to_string(),
                    });
                }
                let tasks_to_supersede = if let Some(val) = call.arguments.get("tasks_to_supersede")
                {
                    let arr = val
                        .as_array()
                        .ok_or_else(|| ActionParseError::InvalidParameter {
                            action: "adapt_strategy",
                            field: "tasks_to_supersede",
                            reason: "expected array of UUID strings".to_string(),
                        })?;
                    let mut tasks = Vec::with_capacity(arr.len());
                    for item in arr {
                        let s =
                            item.as_str()
                                .ok_or_else(|| ActionParseError::InvalidParameter {
                                    action: "adapt_strategy",
                                    field: "tasks_to_supersede",
                                    reason: "expected string UUID".to_string(),
                                })?;
                        let u = s.parse::<uuid::Uuid>().map_err(|e| {
                            ActionParseError::InvalidParameter {
                                action: "adapt_strategy",
                                field: "tasks_to_supersede",
                                reason: format!("malformed UUID '{s}': {e}"),
                            }
                        })?;
                        tasks.push(TaskId::from(u));
                    }
                    tasks
                } else {
                    Vec::new()
                };
                let new_tasks = if let Some(val) = call.arguments.get("new_tasks") {
                    let arr = val
                        .as_array()
                        .ok_or_else(|| ActionParseError::InvalidParameter {
                            action: "adapt_strategy",
                            field: "new_tasks",
                            reason: "expected array of strings".to_string(),
                        })?;
                    let mut strings = Vec::with_capacity(arr.len());
                    for item in arr {
                        let s =
                            item.as_str()
                                .ok_or_else(|| ActionParseError::InvalidParameter {
                                    action: "adapt_strategy",
                                    field: "new_tasks",
                                    reason: "expected task description string".to_string(),
                                })?;
                        strings.push(s.to_string());
                    }
                    strings
                } else {
                    Vec::new()
                };
                Ok(Self::Replan {
                    reason: reason.to_string(),
                    tasks_to_supersede,
                    new_tasks,
                })
            }
            "write_file" => {
                let path_str = call.arguments.get("path").and_then(|v| v.as_str()).ok_or(
                    ActionParseError::MissingRequiredField {
                        action: "write_file",
                        field: "path",
                    },
                )?;
                if path_str.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "write_file",
                        field: "path",
                        reason: "path cannot be empty".to_string(),
                    });
                }
                let content_str = call
                    .arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "write_file",
                        field: "content",
                    })?;
                let description = call
                    .arguments
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                Ok(Self::WriteChange {
                    path: PathBuf::from(path_str),
                    content: content_str.to_string(),
                    description,
                })
            }
            "run_verification" => {
                let tier = if let Some(val) = call.arguments.get("tier") {
                    if val.is_null() {
                        None
                    } else {
                        let n = val
                            .as_u64()
                            .ok_or_else(|| ActionParseError::InvalidParameter {
                                action: "run_verification",
                                field: "tier",
                                reason: "tier must be an integer between 1 and 3".to_string(),
                            })?;
                        if !(1..=3).contains(&n) {
                            return Err(ActionParseError::InvalidParameter {
                                action: "run_verification",
                                field: "tier",
                                reason: format!("tier {} exceeds supported range [1, 3]", n),
                            });
                        }
                        Some(n as u8)
                    }
                } else {
                    None
                };
                let target = call
                    .arguments
                    .get("target")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                Ok(Self::RunVerification { tier, target })
            }
            "ask_user" => {
                let question = call
                    .arguments
                    .get("question")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "ask_user",
                        field: "question",
                    })?;
                if question.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "ask_user",
                        field: "question",
                        reason: "question cannot be empty".to_string(),
                    });
                }
                let options =
                    if let Some(val) = call.arguments.get("options") {
                        let arr =
                            val.as_array()
                                .ok_or_else(|| ActionParseError::InvalidParameter {
                                    action: "ask_user",
                                    field: "options",
                                    reason: "expected array of options".to_string(),
                                })?;
                        let mut opts = Vec::with_capacity(arr.len());
                        for (idx, item) in arr.iter().enumerate() {
                            let opt: ActionUserOption = serde_json::from_value(item.clone())
                                .map_err(|e| ActionParseError::InvalidParameter {
                                    action: "ask_user",
                                    field: "options",
                                    reason: format!("malformed option at index {idx}: {e}"),
                                })?;
                            opts.push(opt);
                        }
                        opts
                    } else {
                        Vec::new()
                    };
                Ok(Self::AskUser {
                    question: question.to_string(),
                    options,
                })
            }
            "request_approval" => {
                let tool_name = call
                    .arguments
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "request_approval",
                        field: "tool_name",
                    })?;
                if tool_name.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "request_approval",
                        field: "tool_name",
                        reason: "tool_name cannot be empty".to_string(),
                    });
                }
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "request_approval",
                        field: "reason",
                    })?;
                if reason.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "request_approval",
                        field: "reason",
                        reason: "reason cannot be empty".to_string(),
                    });
                }
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
                Ok(Self::RequestApproval {
                    tool_name: tool_name.to_string(),
                    parameters,
                    risk,
                    reason: reason.to_string(),
                })
            }
            "create_task" => {
                let title = call.arguments.get("title").and_then(|v| v.as_str()).ok_or(
                    ActionParseError::MissingRequiredField {
                        action: "create_task",
                        field: "title",
                    },
                )?;
                if title.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "create_task",
                        field: "title",
                        reason: "title cannot be empty".to_string(),
                    });
                }
                let description = call
                    .arguments
                    .get("description")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "create_task",
                        field: "description",
                    })?;
                let role = if let Some(val) = call.arguments.get("role") {
                    if val.is_null() {
                        None
                    } else {
                        let r_str =
                            val.as_str()
                                .ok_or_else(|| ActionParseError::InvalidParameter {
                                    action: "create_task",
                                    field: "role",
                                    reason: "expected string role".to_string(),
                                })?;
                        Some(AgentRole::new(r_str))
                    }
                } else {
                    None
                };
                let dependencies = if let Some(val) = call.arguments.get("dependencies") {
                    let arr = val
                        .as_array()
                        .ok_or_else(|| ActionParseError::InvalidParameter {
                            action: "create_task",
                            field: "dependencies",
                            reason: "expected array of UUID strings".to_string(),
                        })?;
                    let mut deps = Vec::with_capacity(arr.len());
                    for item in arr {
                        let s =
                            item.as_str()
                                .ok_or_else(|| ActionParseError::InvalidParameter {
                                    action: "create_task",
                                    field: "dependencies",
                                    reason: "expected string UUID".to_string(),
                                })?;
                        let u = s.parse::<uuid::Uuid>().map_err(|e| {
                            ActionParseError::InvalidParameter {
                                action: "create_task",
                                field: "dependencies",
                                reason: format!("malformed UUID '{s}': {e}"),
                            }
                        })?;
                        deps.push(TaskId::from(u));
                    }
                    deps
                } else {
                    Vec::new()
                };
                Ok(Self::CreateTask {
                    title: title.to_string(),
                    description: description.to_string(),
                    role,
                    dependencies,
                })
            }
            "supersede_task" => {
                let task_id_str = call
                    .arguments
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "supersede_task",
                        field: "task_id",
                    })?;
                let task_id_uuid = task_id_str.parse::<uuid::Uuid>().map_err(|e| {
                    ActionParseError::InvalidParameter {
                        action: "supersede_task",
                        field: "task_id",
                        reason: format!("malformed UUID '{task_id_str}': {e}"),
                    }
                })?;
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .ok_or(ActionParseError::MissingRequiredField {
                        action: "supersede_task",
                        field: "reason",
                    })?;
                if reason.trim().is_empty() {
                    return Err(ActionParseError::InvalidParameter {
                        action: "supersede_task",
                        field: "reason",
                        reason: "reason cannot be empty".to_string(),
                    });
                }
                Ok(Self::SupersedeTask {
                    task_id: TaskId::from(task_id_uuid),
                    reason: reason.to_string(),
                })
            }
            _ => Ok(Self::CallTool {
                tool_name: call.name.clone(),
                parameters: call.arguments.clone(),
            }),
        }
    }

    /// canonical translation from a raw model tool call to strongly typed agent action.
    pub fn from_tool_call(
        call: &crate::model::types::ModelToolCall,
    ) -> Result<Self, ActionParseError> {
        Self::try_from_tool_call(call)
    }

    /// canonical translation from a model proposal to one or more strongly typed agent actions.
    pub fn from_model_proposal(proposal: &crate::model::types::ModelProposal) -> Vec<Self> {
        match proposal {
            crate::model::types::ModelProposal::ToolCalls { calls } => calls
                .iter()
                .filter_map(|c| Self::from_tool_call(c).ok())
                .collect(),
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
