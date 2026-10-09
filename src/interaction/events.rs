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

    /// Authoritative persisted conversation history loaded for a resumed session.
    SessionHistoryLoaded {
        session_id: SessionId,
        turns: Vec<crate::interaction::session::ConversationTurn>,
    },

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

    /// Independent verification gate started.
    VerificationStarted {
        verification_id: String,
        mission_id: String,
        target: String,
    },

    /// Independent verification gate passed successfully.
    VerificationPassed {
        summary: String,
        #[serde(default)]
        verification_id: Option<String>,
        #[serde(default)]
        check: Option<crate::tui::model::TuiVerificationCheck>,
    },

    /// Independent verification gate failed.
    VerificationFailed {
        summary: String,
        #[serde(default)]
        verification_id: Option<String>,
        #[serde(default)]
        check: Option<crate::tui::model::TuiVerificationCheck>,
    },

    /// Operator approval requested due to policy evaluation.
    ApprovalRequested {
        request_id: String,
        tool_name: String,
        details: String,
        #[serde(default)]
        risk_tier: Option<String>,
        #[serde(default)]
        parameters_summary: Option<String>,
        #[serde(default)]
        agent_role: Option<String>,
        #[serde(default)]
        timeout_seconds: Option<u64>,
    },

    /// Operator approval resolved.
    ApprovalResolved { request_id: String, approved: bool },

    /// Agent lifecycle events (Problem 2).
    AgentSpawned {
        agent_id: String,
        mission_id: String,
        role: String,
    },
    AgentStarted {
        agent_id: String,
        mission_id: String,
        task_id: String,
    },
    AgentStepCompleted {
        agent_id: String,
        task_id: String,
        step_number: u32,
        steps_remaining: u32,
    },
    AgentHandoffRecorded {
        handoff_id: String,
        mission_id: String,
        source_agent_id: String,
        target_role: String,
        reason: String,
    },
    AgentCompleted {
        agent_id: String,
        mission_id: String,
        summary: String,
    },
    AgentFailed {
        agent_id: String,
        mission_id: String,
        error: String,
    },
    AgentCancelled {
        agent_id: String,
        mission_id: String,
        reason: String,
    },

    /// Job lifecycle events (Problem 2).
    JobStarted {
        job_id: String,
        mission_id: String,
        job_type: String,
    },
    JobCompleted {
        job_id: String,
        mission_id: String,
        artifacts: Vec<String>,
    },
    JobFailed {
        job_id: String,
        mission_id: String,
        error: String,
    },

    /// Checkpoint lifecycle events (Problem 2).
    CheckpointCreated {
        checkpoint_id: String,
        mission_id: String,
        description: String,
    },
    CheckpointRestored {
        checkpoint_id: String,
        mission_id: String,
    },

    /// Recovery lifecycle events (Problem 2 & 6).
    RecoveryAttempted {
        recovery_id: String,
        mission_id: String,
        strategy: String,
        success: bool,
    },

    /// Live budget telemetry update (Problem 6).
    BudgetSnapshotUpdated {
        snapshot: Box<crate::tui::model::TuiBudgetSnapshot>,
    },

    /// Task was blocked waiting on dependencies or policy.
    TaskBlocked {
        task_id: String,
        mission_id: String,
        reason: String,
    },

    /// Task was unblocked and is ready for execution.
    TaskUnblocked { task_id: String, mission_id: String },

    /// Task retry scheduled after an attempt failure.
    TaskRetryScheduled {
        task_id: String,
        mission_id: String,
        attempt: u32,
        retry_delay_ms: u64,
    },

    /// Task entered needs_review state.
    TaskNeedsReview {
        task_id: String,
        mission_id: String,
        reason: String,
    },

    /// Task attempt failed before retry or terminal failure.
    TaskAttemptFailed {
        task_id: String,
        mission_id: String,
        attempt: u32,
        error: String,
    },

    /// Task was skipped due to conditional execution or upstream failure.
    TaskSkipped {
        task_id: String,
        mission_id: String,
        reason: String,
    },

    /// Task was newly created in the DAG.
    TaskCreated {
        task_id: String,
        mission_id: String,
        description: String,
    },

    /// Task operator review concluded.
    TaskReviewed {
        task_id: String,
        mission_id: String,
        approved: bool,
        reviewer: String,
    },

    /// Task was superseded by replacement task.
    TaskSuperseded {
        task_id: String,
        mission_id: String,
        replacement_task_id: Option<String>,
    },

    /// Critical path recalculated for a task graph.
    CriticalPathRecalculated {
        graph_id: String,
        critical_tasks: Vec<String>,
        projected_duration_secs: u64,
    },

    /// Concurrency resource leased by a task.
    ResourceLeased {
        lease_id: String,
        task_id: String,
        resource_key: String,
        lock_mode: String,
    },

    /// Concurrency resource released by a task.
    ResourceReleased {
        lease_id: String,
        task_id: String,
        resource_key: String,
    },

    /// Concurrency resource revoked from a task.
    ResourceRevoked {
        lease_id: String,
        task_id: String,
        resource_key: String,
        reason: String,
    },

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

    /// Candidate plan revision ready for explicit operator review (Problem 3).
    PlanForReview {
        session_id: String,
        revision: u32,
        plan_id: String,
        objective: String,
        task_count: usize,
        content_hash: Option<String>,
        #[serde(default)]
        plan_markdown: Option<String>,
        #[serde(default)]
        tasks: Vec<crate::kernel::plan::CandidateTask>,
    },

    /// Candidate task set ready for explicit operator review (Problem 3).
    TasksForReview {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        task_count: usize,
        content_hash: Option<String>,
        #[serde(default)]
        task_markdown: Option<String>,
        #[serde(default)]
        tasks: Vec<crate::kernel::plan::CandidateTask>,
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
        cost_usd: Option<f64>,
        cost_provenance: crate::model::types::CostProvenance,
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

    /// Diagnostic doctor probe report evaluated.
    DoctorReportUpdated {
        checks: Vec<crate::tui::model::TuiDoctorCheck>,
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
            Self::SessionHistoryLoaded { session_id, turns } => {
                format!(
                    "● Loaded {} conversation turns for session {session_id}",
                    turns.len()
                )
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
            Self::VerificationStarted {
                verification_id,
                target,
                ..
            } => {
                format!("🔍 Verification started: {verification_id} ({target})")
            }
            Self::VerificationPassed { summary, .. } => {
                format!("✔ Verification passed: {summary}")
            }
            Self::VerificationFailed { summary, .. } => {
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
            Self::AgentSpawned { agent_id, role, .. } => {
                format!("👤 Agent spawned: {role} ({agent_id})")
            }
            Self::AgentStarted {
                agent_id, task_id, ..
            } => {
                format!("▶ Agent started: {agent_id} on task {task_id}")
            }
            Self::AgentStepCompleted {
                agent_id,
                step_number,
                steps_remaining,
                ..
            } => {
                format!("Step {step_number} completed by {agent_id} ({steps_remaining} remaining)")
            }
            Self::AgentHandoffRecorded {
                source_agent_id,
                target_role,
                reason,
                ..
            } => {
                format!("Handoff from {source_agent_id} to {target_role}: {reason}")
            }
            Self::AgentCompleted {
                agent_id, summary, ..
            } => {
                format!("✔ Agent {agent_id} completed: {summary}")
            }
            Self::AgentFailed {
                agent_id, error, ..
            } => {
                format!("✘ Agent {agent_id} failed: {error}")
            }
            Self::AgentCancelled {
                agent_id, reason, ..
            } => {
                format!("○ Agent {agent_id} cancelled: {reason}")
            }
            Self::JobStarted {
                job_id, job_type, ..
            } => {
                format!("▶ Job started: {job_id} ({job_type})")
            }
            Self::JobCompleted {
                job_id, artifacts, ..
            } => {
                format!("✔ Job completed: {job_id} ({} artifacts)", artifacts.len())
            }
            Self::JobFailed { job_id, error, .. } => {
                format!("✘ Job failed: {job_id} — {error}")
            }
            Self::CheckpointCreated {
                checkpoint_id,
                description,
                ..
            } => {
                format!("💾 Checkpoint created: {checkpoint_id} — {description}")
            }
            Self::CheckpointRestored { checkpoint_id, .. } => {
                format!("↩ Checkpoint restored: {checkpoint_id}")
            }
            Self::RecoveryAttempted {
                recovery_id,
                strategy,
                success,
                ..
            } => {
                let status = if *success { "succeeded" } else { "failed" };
                format!("🔄 Recovery {recovery_id} ({strategy}): {status}")
            }
            Self::BudgetSnapshotUpdated { snapshot } => {
                format!(
                    "💰 Budget updated: {}¢ / {}¢",
                    snapshot.consumed_cents, snapshot.allocated_cents
                )
            }
            Self::TaskBlocked {
                task_id, reason, ..
            } => {
                format!("⛔ Task {task_id} blocked: {reason}")
            }
            Self::TaskUnblocked { task_id, .. } => {
                format!("▶ Task {task_id} unblocked")
            }
            Self::TaskRetryScheduled {
                task_id,
                attempt,
                retry_delay_ms,
                ..
            } => {
                format!("🔁 Task {task_id} retry #{attempt} in {retry_delay_ms}ms")
            }
            Self::TaskNeedsReview {
                task_id, reason, ..
            } => {
                format!("📋 Task {task_id} needs review: {reason}")
            }
            Self::CriticalPathRecalculated {
                graph_id,
                projected_duration_secs,
                ..
            } => {
                format!(
                    "⏱ Critical path recalculated for graph {graph_id} ({projected_duration_secs}s)"
                )
            }
            Self::ResourceLeased {
                task_id,
                resource_key,
                lock_mode,
                ..
            } => {
                format!("🔒 Resource {resource_key} leased to task {task_id} ({lock_mode})")
            }
            Self::ResourceReleased {
                task_id,
                resource_key,
                ..
            } => {
                format!("🔓 Resource {resource_key} released by task {task_id}")
            }
            Self::ResourceRevoked {
                task_id,
                resource_key,
                reason,
                ..
            } => {
                format!("⛔ Resource {resource_key} revoked from task {task_id}: {reason}")
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
                cost_usd,
                cost_provenance,
                ..
            } => {
                let cost_str = match (cost_usd, cost_provenance) {
                    (Some(u), crate::model::types::CostProvenance::Authoritative) => {
                        format!("${:.4} (authoritative)", u)
                    }
                    (Some(u), crate::model::types::CostProvenance::Estimated) => {
                        format!("${:.4} (estimated)", u)
                    }
                    _ => "cost unknown".to_string(),
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
            Self::TaskAttemptFailed {
                task_id,
                attempt,
                error,
                ..
            } => {
                format!("⚠ Task {task_id} attempt #{attempt} failed: {error}")
            }
            Self::TaskSkipped {
                task_id, reason, ..
            } => {
                format!("↷ Task {task_id} skipped: {reason}")
            }
            Self::TaskCreated {
                task_id,
                description,
                ..
            } => {
                format!("＋ Task created: {task_id} — {description}")
            }
            Self::TaskReviewed {
                task_id,
                approved,
                reviewer,
                ..
            } => {
                let decision = if *approved { "approved" } else { "rejected" };
                format!("⚖ Task {task_id} {decision} by {reviewer}")
            }
            Self::TaskSuperseded {
                task_id,
                replacement_task_id,
                ..
            } => match replacement_task_id {
                Some(r) => format!("⇋ Task {task_id} superseded by {r}"),
                None => format!("⇋ Task {task_id} superseded"),
            },
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
            Self::DoctorReportUpdated { checks } => {
                format!("🩺 Doctor diagnostics: {} probe(s) updated", checks.len())
            }
        }
    }

    /// Semantic delivery classification defining priority and backpressure under bounded-channel saturation.
    pub fn delivery_class(&self) -> EventDeliveryClass {
        match self {
            Self::ApprovalRequested { .. }
            | Self::ApprovalResolved { .. }
            | Self::TaskCompleted { .. }
            | Self::TaskFailed { .. }
            | Self::TaskCancelled { .. }
            | Self::TaskStarted { .. }
            | Self::TaskBlocked { .. }
            | Self::TaskUnblocked { .. }
            | Self::TaskRetryScheduled { .. }
            | Self::TaskNeedsReview { .. }
            | Self::TaskAttemptFailed { .. }
            | Self::TaskSkipped { .. }
            | Self::TaskCreated { .. }
            | Self::TaskReviewed { .. }
            | Self::TaskSuperseded { .. }
            | Self::VerificationStarted { .. }
            | Self::VerificationPassed { .. }
            | Self::VerificationFailed { .. }
            | Self::AuthorizationRequired { .. }
            | Self::ExecutionReady { .. }
            | Self::LifecycleTerminated { .. }
            | Self::MissionStateChanged { .. }
            | Self::Completion { .. }
            | Self::Error { .. }
            | Self::CheckpointCreated { .. }
            | Self::CheckpointRestored { .. }
            | Self::RecoveryAttempted { .. }
            | Self::JobStarted { .. }
            | Self::JobCompleted { .. }
            | Self::JobFailed { .. }
            | Self::AgentSpawned { .. }
            | Self::AgentStarted { .. }
            | Self::AgentCompleted { .. }
            | Self::AgentFailed { .. }
            | Self::AgentCancelled { .. }
            | Self::AgentHandoffRecorded { .. }
            | Self::DiscoveryRequired { .. }
            | Self::PlanForReview { .. }
            | Self::TasksForReview { .. }
            | Self::TasksMaterialized { .. }
            | Self::SessionStarted { .. }
            | Self::SessionResumed { .. }
            | Self::SessionHistoryLoaded { .. } => EventDeliveryClass::Critical,

            Self::BudgetSnapshotUpdated { .. }
            | Self::WorkflowSnapshotUpdated { .. }
            | Self::DoctorReportUpdated { .. }
            | Self::ModelUsageUpdated { .. }
            | Self::GitStateChanged { .. }
            | Self::CriticalPathRecalculated { .. }
            | Self::ResourceLeased { .. }
            | Self::ResourceReleased { .. }
            | Self::ResourceRevoked { .. }
            | Self::AgentStepCompleted { .. } => EventDeliveryClass::ReplaceableTelemetry,

            Self::AssistantStarted { .. }
            | Self::AssistantDelta { .. }
            | Self::AssistantFinished { .. }
            | Self::AssistantFailed { .. }
            | Self::AssistantOutput { .. }
            | Self::ModelActivity { .. }
            | Self::ToolStarted { .. }
            | Self::ToolCompleted { .. }
            | Self::CommandOutput { .. }
            | Self::ConfigurationUpdated { .. } => EventDeliveryClass::Informational,
        }
    }
}

/// Semantic classification of interaction events defining delivery priority and backpressure behavior under channel saturation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventDeliveryClass {
    /// Critical state transitions that must never be silently discarded.
    /// Under saturation, the bridge must use bounded retry/backpressure or outbox reconciliation.
    Critical,
    /// Replaceable telemetry snapshots where newer values supersede older ones.
    /// Under saturation, telemetry can be coalesced so only the latest snapshot is retained.
    ReplaceableTelemetry,
    /// Informational or high-frequency streaming messages where dropping under extreme backpressure is acceptable.
    Informational,
}
