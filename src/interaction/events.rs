//! User-facing event stream abstraction decoupling CLI/TUI from internal scheduling (PRD §01, CLI-02).
//!
//! Maps internal domain events and model/tool executions into structured, human- and machine-readable
//! interaction events without scheduler coupling.

use serde::{Deserialize, Serialize};

use crate::ids::{MissionId, SessionId};

/// User-facing interaction event stream item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractionEvent {
    /// Interactive session initialized or attached.
    SessionStarted { session_id: SessionId },

    /// Session resumed from disk.
    SessionResumed { session_id: SessionId },

    /// Natural-language model reasoning or activity indicator.
    ModelActivity { text: String },

    /// An autonomous agent initiated a tool execution.
    ToolStarted {
        call_id: String,
        tool_name: String,
        parameters: serde_json::Value,
    },

    /// An autonomous tool execution concluded with output.
    ToolCompleted {
        call_id: String,
        tool_name: String,
        success: bool,
        output_preview: String,
    },

    /// Model streaming assistant output text to the user.
    AssistantOutput { text: String },

    /// Model started streaming an assistant message turn.
    AssistantStarted { message_id: String },

    /// Model incremental assistant text chunk delta.
    AssistantDelta { message_id: String, delta: String },

    /// Model completed streaming the assistant message turn.
    AssistantFinished { message_id: String },

    /// Model stream failed or was interrupted.
    AssistantFailed { message_id: String, error: String },

    /// Independent verification gate passed successfully.
    VerificationPassed { summary: String },

    /// Independent verification gate failed.
    VerificationFailed { summary: String },

    /// Operator approval requested due to policy evaluation.
    ApprovalRequested {
        request_id: String,
        tool_name: String,
        details: String,
    },

    /// Operator approval resolved.
    ApprovalResolved { request_id: String, approved: bool },

    /// Mission status changed (e.g. Started, Paused, Completed, Failed).
    MissionStateChanged {
        mission_id: MissionId,
        status: String,
    },

    /// Informational output or command result text.
    CommandOutput { text: String },

    /// An error occurred during interaction or execution.
    Error { message: String },

    /// Autonomous task or mission reached truthful verified completion.
    Completion { summary: String },

    /// Governed lifecycle needs discovery answers before planning.
    DiscoveryRequired {
        session_id: String,
        questions: Vec<crate::workflow::genesis::DynamicQuestion>,
    },

    /// Candidate plan revision ready for explicit operator review.
    PlanForReview {
        session_id: String,
        revision: u32,
        plan_id: String,
        objective: String,
        task_count: usize,
        content_hash: Option<String>,
    },

    /// Candidate task set ready for explicit operator review.
    TasksForReview {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        task_count: usize,
        content_hash: Option<String>,
    },

    /// Plan and tasks accepted; explicit execution authorization requested.
    AuthorizationRequired {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        message: String,
    },

    /// Operator authorized execution of exact artifact revisions.
    ExecutionReady {
        session_id: String,
        authorization_id: String,
        plan_revision: u32,
        task_revision: u32,
    },

    /// Governed lifecycle reached a terminal stage.
    LifecycleTerminated {
        session_id: String,
        stage: String,
        reason: String,
    },

    /// Authoritative model usage updated.
    ModelUsageUpdated {
        invocation_id: Option<String>,
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
        cost_cents: Option<u64>,
    },

    /// Authoritative git repository / worktree state changed.
    GitStateChanged {
        workspace_branch: String,
        execution_branch: Option<String>,
        is_clean: bool,
    },

    /// Authoritative task graph materialized with complete tasks.
    TasksMaterialized {
        graph_id: String,
        revision: u32,
        tasks: Vec<crate::events::types::TaskSummary>,
    },

    /// Authoritative task execution started.
    TaskStarted {
        task_id: String,
        mission_id: String,
        agent_id: String,
    },

    /// Authoritative task execution completed.
    TaskCompleted {
        task_id: String,
        mission_id: String,
        result: String,
    },

    /// Authoritative task execution failed.
    TaskFailed {
        task_id: String,
        mission_id: String,
        error: String,
    },

    /// Authoritative task execution cancelled.
    TaskCancelled {
        task_id: String,
        mission_id: String,
        reason: String,
    },

    /// Active runtime model/provider/profile configuration updated.
    ConfigurationUpdated {
        model: String,
        provider: String,
        profile: Option<String>,
    },

    /// Workflow execution state snapshot updated from runtime authority.
    WorkflowSnapshotUpdated {
        snapshot: Box<crate::workflow::engine::WorkflowExecutionSnapshot>,
    },
}

impl InteractionEvent {
    /// Format event for clean human-readable CLI display.
    pub fn format_human(&self) -> String {
        match self {
            Self::SessionStarted { session_id } => {
                format!("● Session started: {session_id}")
            }
            Self::SessionResumed { session_id } => {
                format!("● Session resumed: {session_id}")
            }
            Self::ModelActivity { text } => {
                format!("⋯ Model: {text}")
            }
            Self::ToolStarted {
                tool_name,
                parameters,
                ..
            } => {
                let params_str = serde_json::to_string(parameters).unwrap_or_default();
                let preview = if params_str.len() > 100 {
                    format!("{}...", &params_str[..100])
                } else {
                    params_str
                };
                format!("⚙ Running tool `{tool_name}`: {preview}")
            }
            Self::ToolCompleted {
                tool_name,
                success,
                output_preview,
                ..
            } => {
                let status_icon = if *success { "✔" } else { "✘" };
                let preview = if output_preview.len() > 140 {
                    format!("{}...", &output_preview[..140])
                } else {
                    output_preview.clone()
                };
                format!("{status_icon} Tool `{tool_name}` finished: {preview}")
            }
            Self::AssistantOutput { text } => text.clone(),
            Self::AssistantStarted { .. } => String::new(),
            Self::AssistantDelta { delta, .. } => delta.clone(),
            Self::AssistantFinished { .. } => String::new(),
            Self::AssistantFailed { error, .. } => format!("✘ Stream failed: {error}"),
            Self::VerificationPassed { summary } => {
                format!("✔ Verification passed: {summary}")
            }
            Self::VerificationFailed { summary } => {
                format!("✘ Verification failed: {summary}")
            }
            Self::ApprovalRequested {
                tool_name, details, ..
            } => {
                format!("⚠ Policy check required for `{tool_name}`: {details}")
            }
            Self::ApprovalResolved { approved, .. } => {
                if *approved {
                    "✔ Action approved by operator".to_string()
                } else {
                    "✘ Action denied by operator".to_string()
                }
            }
            Self::MissionStateChanged { status, .. } => {
                format!("◆ Mission state: {status}")
            }
            Self::CommandOutput { text } => text.clone(),
            Self::Error { message } => {
                format!("✘ Error: {message}")
            }
            Self::Completion { summary } => {
                format!("🎉 Mission completed: {summary}")
            }
            Self::DiscoveryRequired {
                session_id,
                questions,
            } => {
                let texts = questions
                    .iter()
                    .map(|q| q.text.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                format!(
                    "◆ Discovery required (session {session_id}): {} question(s): {}",
                    questions.len(),
                    texts
                )
            }
            Self::PlanForReview {
                revision,
                plan_id,
                objective,
                ..
            } => {
                format!("[PLAN REVIEW] Revision {revision} (Plan ID: {plan_id}): {objective}")
            }
            Self::TasksForReview {
                plan_revision,
                task_revision,
                task_count,
                ..
            } => {
                format!(
                    "[TASK REVIEW] Plan rev {plan_revision}, task rev {task_revision}: {task_count} tasks"
                )
            }
            Self::AuthorizationRequired {
                plan_revision,
                task_revision,
                message,
                ..
            } => {
                format!(
                    "[EXECUTION AUTHORIZATION REQUIRED] Plan rev {plan_revision}, task rev {task_revision}: {message}"
                )
            }
            Self::ExecutionReady {
                authorization_id,
                plan_revision,
                task_revision,
                ..
            } => {
                format!(
                    "[AUTHORIZED — NOT YET EXECUTING] auth {authorization_id}, plan rev {plan_revision}, task rev {task_revision}"
                )
            }
            Self::LifecycleTerminated { stage, reason, .. } => {
                format!("[Lifecycle terminated] stage '{stage}': {reason}")
            }
            Self::ModelUsageUpdated {
                total_tokens,
                cost_cents,
                ..
            } => {
                let cost_str = match cost_cents {
                    Some(c) => format!("${:.2}", *c as f64 / 100.0),
                    None => "cost n/a".to_string(),
                };
                format!("📊 Model usage: {total_tokens} tok ({cost_str})")
            }
            Self::GitStateChanged {
                workspace_branch,
                execution_branch,
                ..
            } => match execution_branch {
                Some(eb) => format!("🌿 Git: {eb} (ws: {workspace_branch})"),
                None => format!("🌿 Git: {workspace_branch}"),
            },
            Self::TasksMaterialized {
                revision, tasks, ..
            } => {
                format!(
                    "📋 Tasks materialized (rev {revision}): {} task(s)",
                    tasks.len()
                )
            }
            Self::TaskStarted {
                task_id, agent_id, ..
            } => {
                format!("▶ Task started: {task_id} (Agent: {agent_id})")
            }
            Self::TaskCompleted {
                task_id, result, ..
            } => {
                let preview = if result.len() > 100 {
                    format!("{}...", &result[..100])
                } else {
                    result.clone()
                };
                format!("✔ Task completed: {task_id} — {preview}")
            }
            Self::TaskFailed { task_id, error, .. } => {
                format!("✘ Task failed: {task_id} — {error}")
            }
            Self::TaskCancelled {
                task_id, reason, ..
            } => {
                format!("○ Task cancelled: {task_id} — {reason}")
            }
            Self::ConfigurationUpdated {
                model,
                provider,
                profile,
            } => {
                let prof = profile.as_deref().unwrap_or("none");
                format!("⚙ Config updated: provider={provider}, model={model}, profile={prof}")
            }
            Self::WorkflowSnapshotUpdated { snapshot } => {
                format!(
                    "⟳ Workflow snapshot: run {} ({}) status {}",
                    snapshot.run.id, snapshot.run.definition_id, snapshot.run.status
                )
            }
        }
    }
}
