//! Asynchronous Runtime Bridge for TUI Cockpit (COP-04, CLI-01, CLI-04, D-15).
//!
//! Connects the synchronous 60 FPS TUI projection layer to the asynchronous
//! `AppRuntime` core without violating Law 9 ("The model proposes. The runtime decides")
//! or Law 15 (Zero database I/O during frame rendering).
//!
//! Responsibilities:
//! - Receives typed `ApplicationAction` from the composer/palette
//! - Dispatches actions to `AppRuntime`, `SqliteSessionRepository`, and `SlashCommandRegistry`
//! - Subscribes to `BroadcastEventBus` and transforms kernel events into `InteractionEvent`s
//! - Streams `InteractionEvent`s back to the TUI update channel

use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_util::sync::CancellationToken;

use crate::error::M31AError;
use crate::events::bus::{EventBus, EventFilter};
use crate::events::types::EventType;
use crate::ids::{MissionId, SessionId};
use crate::interaction::action::ApplicationAction;
use crate::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use crate::interaction::continuation::{ActiveExecution, emit};
use crate::interaction::events::InteractionEvent;
use crate::interaction::session::{
    ConversationTurn, Session, SessionState, SqliteSessionRepository,
};
use crate::planning::review::{PlanRevision, PreExecutionResponse, TaskRevision};
use crate::runtime::AppRuntime;
use crate::tui::approval::ApprovalDecision;

/// Asynchronous handle held by the TUI to communicate with the runtime.
pub struct TuiRuntimeBridge {
    action_tx: UnboundedSender<ApplicationAction>,
    event_rx: Option<UnboundedReceiver<InteractionEvent>>,
    pub session_id: Option<SessionId>,
    pub workspace_root: PathBuf,
}

impl TuiRuntimeBridge {
    /// Send an application action to the runtime worker without blocking.
    pub fn send_action(&self, action: ApplicationAction) -> bool {
        self.action_tx.send(action).is_ok()
    }

    /// Take the event receiver for passing to `TuiApplication`.
    pub fn take_event_receiver(&mut self) -> Option<UnboundedReceiver<InteractionEvent>> {
        self.event_rx.take()
    }

    /// Clone sender for dispatching actions from modal dialogs or shortcuts.
    pub fn sender(&self) -> UnboundedSender<ApplicationAction> {
        self.action_tx.clone()
    }

    /// Spawn a new background worker bridge attached to the given `AppRuntime`.
    pub async fn spawn(
        runtime: Arc<AppRuntime>,
        resume_session_id: Option<SessionId>,
    ) -> Result<(Self, tokio::task::JoinHandle<()>), M31AError> {
        let (action_tx, action_rx) = unbounded_channel();
        let (event_tx, event_rx) = unbounded_channel();
        let workspace_root = runtime.workspace_root().to_path_buf();

        let pool = runtime.pool().clone();
        let session_repo = SqliteSessionRepository::new(pool);

        // 1. Subscribe to the canonical EventBus BEFORE session hydration
        // so no live events published during startup or hydration can be lost.
        let kernel_rx = runtime.event_bus().subscribe(EventFilter::all()).await;

        // 2. Initialize or resume durable session (fail closed on
        // cross-workspace resume: never attach a foreign-workspace session
        // to this runtime's authority graph).
        let (session, was_resume) = if let Some(sid) = resume_session_id {
            let s = match session_repo.get_session(sid).await? {
                Some(s) => s,
                None => {
                    return Err(M31AError::internal(format!("session '{sid}' not found")));
                }
            };
            // Cross-workspace protection (SES-01, P0-SEC-01):
            let sess_ws = crate::init::instance::canonicalize_workspace_root(&s.workspace_root);
            let runtime_ws = crate::init::instance::canonicalize_workspace_root(&workspace_root);
            if sess_ws != runtime_ws {
                return Err(M31AError::internal(format!(
                    "refusing to resume session '{sid}': session workspace '{}' != runtime workspace '{}'",
                    s.workspace_root.display(),
                    workspace_root.display()
                )));
            }
            (Some(s), true)
        } else {
            // Fresh launch: do NOT automatically resume an arbitrary previous session
            // and do NOT create a new session before operator intent.
            (None, false)
        };

        let active_sid = session.as_ref().map(|s| s.id);

        if was_resume {
            if let Some(sid) = active_sid {
                emit(
                    &event_tx,
                    InteractionEvent::SessionResumed { session_id: sid },
                );
            }
        }

        // Emit initial authoritative runtime configuration
        emit(
            &event_tx,
            InteractionEvent::ConfigurationUpdated {
                model: runtime.config().active_model.clone(),
                provider: runtime.config().active_provider.clone(),
                profile: runtime.config().active_profile.clone(),
            },
        );

        // 3. Hydrate persisted session truth ONLY on explicit resume
        if let Some(ref s) = session {
            for ev in hydrate_session_state(&runtime, &session_repo, s).await {
                emit(&event_tx, ev);
            }
        }

        let worker_runtime = runtime.clone();
        let worker_ws = workspace_root.clone();

        let join_handle = tokio::spawn(async move {
            run_bridge_worker(
                worker_runtime,
                worker_ws,
                session_repo,
                session,
                action_rx,
                event_tx,
                kernel_rx,
            )
            .await;
        });

        let bridge = Self {
            action_tx,
            event_rx: Some(event_rx),
            session_id: active_sid,
            workspace_root,
        };

        Ok((bridge, join_handle))
    }
}

/// Hydrate persisted session truth at bridge startup.
///
/// Returns interaction events describing durable state; never executes,
/// never authorizes, never mutates. The projection combines these with live
/// events subscribed afterwards.
async fn hydrate_session_state(
    runtime: &Arc<AppRuntime>,
    session_repo: &SqliteSessionRepository,
    session: &Session,
) -> Vec<InteractionEvent> {
    let mut out = Vec::new();
    let sid_str = session.id.to_string();

    // 1. Persisted conversation depth (truthful restore banner, not replay).
    match session_repo.get_conversation(session.id).await {
        Ok(turns) if !turns.is_empty() => {
            out.push(InteractionEvent::SessionHistoryLoaded {
                session_id: session.id,
                turns: turns.clone(),
            });
            out.push(InteractionEvent::CommandOutput {
                text: format!(
                    "Restored persisted session {} with {} conversation turns.",
                    session.id,
                    turns.len()
                ),
            });
        }
        Err(e) => {
            out.push(InteractionEvent::Error {
                message: format!("Session hydration: conversation load failed: {e}"),
            });
        }
        _ => {}
    }

    // 2. Authoritative lifecycle stage via the canonical coordinator.
    let coordinator = runtime.create_pre_execution_coordinator();
    if let Ok(Some(_)) = coordinator
        .lifecycle_repo()
        .load_lifecycle_state(&sid_str)
        .await
    {
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            coordinator.resume_session(&sid_str),
        )
        .await
        {
            Ok(Ok(resp)) => response_to_hydration_events(resp, &mut out),
            Ok(Err(e)) => {
                out.push(InteractionEvent::Error {
                    message: format!("Session hydration: lifecycle resume failed: {e}"),
                });
            }
            Err(_) => {
                out.push(InteractionEvent::Error {
                    message: "Session hydration: lifecycle resume timed out after 10s".to_string(),
                });
            }
        }
    }

    // 3. Authoritative Git state
    if let Ok(status) = runtime.git_service().status().await {
        out.push(InteractionEvent::GitStateChanged {
            workspace_branch: if status.branch.is_empty() {
                "N/A".to_string()
            } else {
                status.branch
            },
            execution_branch: None,
            is_clean: status.is_clean,
        });
    }

    // 4. Authoritative task graph state if session has active mission or latest active graph exists
    use crate::persistence::sqlite::repositories::TaskGraphRepository;
    let graph_repo = crate::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(
        runtime.pool().clone(),
    );
    let graph_opt = if let Some(mid) = session.active_mission_id {
        graph_repo.get_active_graph(mid).await.unwrap_or(None)
    } else {
        None
    };
    if let Some(graph) = graph_opt {
        out.push(InteractionEvent::TasksMaterialized {
            graph_id: graph.id.to_string(),
            revision: graph.revision,
            tasks: graph.task_summaries(),
        });
    }

    // 5. Authoritative model usage telemetry
    let inv_repo = crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
        runtime.pool().clone(),
    );
    if let Ok(records) = inv_repo.get_all_invocations().await {
        for r in records {
            let cost_cents = r.cost_usd.map(|usd| (usd * 100.0).round() as u64);
            out.push(InteractionEvent::ModelUsageUpdated {
                invocation_id: Some(r.id.to_string()),
                prompt_tokens: r.prompt_tokens as u64,
                completion_tokens: r.completion_tokens as u64,
                total_tokens: r.total_tokens as u64,
                cost_cents,
                cost_usd: r.cost_usd,
                cost_provenance: r.cost_provenance,
            });
        }
    }

    out
}

/// Map a canonical `PreExecutionResponse` to hydration cards without executing.
fn response_to_hydration_events(resp: PreExecutionResponse, out: &mut Vec<InteractionEvent>) {
    match resp {
        PreExecutionResponse::QuestionsRequired {
            session_id,
            questions,
        } => {
            out.push(InteractionEvent::DiscoveryRequired {
                session_id,
                questions,
            });
        }
        PreExecutionResponse::PlanForReview {
            session_id,
            revision,
        } => {
            let content_hash = PlanRevision::compute_content_hash(&revision.content);
            let md = revision.to_markdown();
            out.push(InteractionEvent::PlanForReview {
                session_id,
                revision: revision.revision,
                plan_id: revision.plan_id.clone(),
                objective: revision.content.objective.clone(),
                task_count: revision.content.tasks.len(),
                content_hash: Some(content_hash),
                plan_markdown: Some(md),
                tasks: revision.content.tasks.clone(),
            });
        }
        PreExecutionResponse::TasksForReview {
            session_id,
            revision,
        } => {
            let content_hash = TaskRevision::compute_tasks_hash(&revision.tasks);
            let md = revision.to_markdown();
            out.push(InteractionEvent::TasksForReview {
                session_id,
                plan_revision: revision.plan_revision,
                task_revision: revision.revision,
                task_count: revision.tasks.len(),
                content_hash: Some(content_hash),
                task_markdown: Some(md),
                tasks: revision.tasks.clone(),
            });
        }
        PreExecutionResponse::AuthorizationRequested {
            session_id,
            plan_revision,
            task_revision,
            message,
        } => {
            out.push(InteractionEvent::AuthorizationRequired {
                session_id,
                plan_revision,
                task_revision,
                message,
            });
        }
        PreExecutionResponse::ReadyToExecute {
            session_id,
            authorization,
            ..
        } => {
            // Hydration surfaces the authorization card but refuses
            // automatic re-execution: continuing requires explicit operator
            // action through the canonical authorization path.
            out.push(InteractionEvent::ExecutionReady {
                session_id: session_id.clone(),
                authorization_id: authorization.id.to_string(),
                plan_revision: authorization.plan_revision,
                task_revision: authorization.task_revision,
            });
            out.push(InteractionEvent::CommandOutput {
                text: "Session was authorized/executing at shutdown. Automatic re-execution is refused; review state and re-authorize to continue.".to_string(),
            });
        }
        PreExecutionResponse::Terminated {
            session_id,
            stage,
            reason,
        } => {
            out.push(InteractionEvent::LifecycleTerminated {
                session_id,
                stage: format!("{stage:?}"),
                reason,
            });
        }
    }
}

/// The asynchronous background worker driving execution and event routing.
async fn run_bridge_worker(
    mut runtime: Arc<AppRuntime>,
    workspace_root: PathBuf,
    session_repo: SqliteSessionRepository,
    mut session: Option<Session>,
    mut action_rx: UnboundedReceiver<ApplicationAction>,
    event_tx: UnboundedSender<InteractionEvent>,
    mut kernel_rx: crate::events::bus::EventReceiver,
) {
    let command_registry = runtime.slash_registry().clone();
    let mut cancel_token = CancellationToken::new();
    let mut active_execution: Option<ActiveExecution> = None;
    let mut tool_names: std::collections::HashMap<crate::ids::ToolCallId, String> =
        std::collections::HashMap::new();

    loop {
        tokio::select! {
            // 1. Process kernel bus events and forward to TUI
            Some(Ok(env)) = kernel_rx.next() => {
                match &env.event_type {
                    EventType::MissionStarted { mission_id, .. } => {
                        emit(&event_tx, InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "running".to_string(),
                        });
                    }
                    EventType::MissionCompleted { mission_id } => {
                        emit(&event_tx, InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "completed".to_string(),
                        });
                    }
                    EventType::MissionFailed { mission_id, reason } => {
                        emit(&event_tx, InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "failed".to_string(),
                        });
                        emit(&event_tx, InteractionEvent::Error {
                            message: format!("Mission failed: {reason}"),
                        });
                    }
                    EventType::MissionPaused { mission_id, .. } => {
                        emit(&event_tx, InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "paused".to_string(),
                        });
                    }
                    EventType::MissionResumed { mission_id, .. } => {
                        emit(&event_tx, InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "running".to_string(),
                        });
                    }
                    EventType::ToolRequested {
                        tool_call_id,
                        tool_name,
                        arguments,
                        ..
                    } => {
                        tool_names.insert(*tool_call_id, tool_name.clone());
                        let params = serde_json::to_value(arguments).unwrap_or_default();
                        emit(&event_tx, InteractionEvent::ToolStarted {
                            call_id: tool_call_id.to_string(),
                            tool_name: tool_name.clone(),
                            parameters: params,
                        });
                    }
                    EventType::ToolCompleted { tool_call_id, result, .. } => {
                        let name = tool_names.remove(tool_call_id).unwrap_or_default();
                        emit(&event_tx, InteractionEvent::ToolCompleted {
                            call_id: tool_call_id.to_string(),
                            tool_name: name,
                            success: true,
                            output_preview: result.clone(),
                        });
                    }
                    EventType::ToolFailed { tool_call_id, error, .. } => {
                        let name = tool_names.remove(tool_call_id).unwrap_or_default();
                        emit(&event_tx, InteractionEvent::ToolCompleted {
                            call_id: tool_call_id.to_string(),
                            tool_name: name,
                            success: false,
                            output_preview: error.clone(),
                        });
                    }
                    EventType::OperatorEscalationRequested {
                        request_id,
                        reason,
                        timeout_seconds,
                        tool_name,
                        parameters_summary,
                        risk_tier,
                        agent_role,
                        ..
                    } => {
                        let final_tool = tool_name
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("Unavailable")
                            .to_string();
                        emit(&event_tx, InteractionEvent::ApprovalRequested {
                            request_id: request_id.clone(),
                            tool_name: final_tool,
                            details: reason.clone(),
                            risk_tier: risk_tier.clone(),
                            parameters_summary: parameters_summary.clone(),
                            agent_role: agent_role.clone(),
                            timeout_seconds: *timeout_seconds,
                        });
                    }
                    EventType::ApprovalResolved { request_id, decision, .. } => {
                        let approved = decision == "ALLOW"
                            || decision == "Approve"
                            || decision == "ApproveOnce"
                            || decision == "ApproveAlways";
                        emit(&event_tx, InteractionEvent::ApprovalResolved {
                            request_id: request_id.clone(),
                            approved,
                        });
                    }
                    // Verification outcomes flow here as interaction cards; the
                    // direct envelope channel carries the same outcome for the
                    // lifecycle projection (no duplicate cards there).
                    EventType::VerificationStarted {
                        verification_id,
                        mission_id,
                        target,
                    } => {
                        emit(&event_tx, InteractionEvent::VerificationStarted {
                            verification_id: verification_id.clone(),
                            mission_id: mission_id.to_string(),
                            target: target.clone(),
                        });
                    }
                    EventType::VerificationCompleted {
                        verification_id,
                        passed,
                        evidence,
                        ..
                    } => {
                        if *passed {
                            emit(&event_tx, InteractionEvent::VerificationPassed {
                                summary: evidence.clone(),
                                verification_id: Some(verification_id.clone()),
                                check: None,
                            });
                        } else {
                            emit(&event_tx, InteractionEvent::VerificationFailed {
                                summary: evidence.clone(),
                                verification_id: Some(verification_id.clone()),
                                check: None,
                            });
                        }
                    }
                    EventType::GitStateChanged {
                        workspace_branch,
                        execution_branch,
                        is_clean,
                    } => {
                        emit(&event_tx, InteractionEvent::GitStateChanged {
                            workspace_branch: workspace_branch.clone(),
                            execution_branch: execution_branch.clone(),
                            is_clean: *is_clean,
                        });
                    }
                    EventType::ModelUsageUpdated {
                        invocation_id,
                        usage,
                        cost_usd,
                        cost_provenance,
                        ..
                    } => {
                        let cost_cents = cost_usd.map(|usd| (usd * 100.0).round() as u64);
                        emit(&event_tx, InteractionEvent::ModelUsageUpdated {
                            invocation_id: invocation_id.map(|u| u.to_string()),
                            prompt_tokens: usage.prompt_tokens as u64,
                            completion_tokens: usage.completion_tokens as u64,
                            total_tokens: usage.total_tokens as u64,
                            cost_cents,
                            cost_usd: *cost_usd,
                            cost_provenance: *cost_provenance,
                        });
                        let budget_snap = crate::tui::model::TuiBudgetSnapshot::from(&runtime.budget_enforcer().snapshot());
                        emit(&event_tx, InteractionEvent::BudgetSnapshotUpdated {
                            snapshot: Box::new(budget_snap),
                        });
                    }
                    EventType::AgentSpawned {
                        agent_id,
                        mission_id,
                        role,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentSpawned {
                            agent_id: agent_id.to_string(),
                            mission_id: mission_id.to_string(),
                            role: role.clone(),
                        });
                    }
                    EventType::AgentStarted {
                        agent_id,
                        mission_id,
                        task_id,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentStarted {
                            agent_id: agent_id.to_string(),
                            mission_id: mission_id.to_string(),
                            task_id: task_id.to_string(),
                        });
                    }
                    EventType::AgentStepCompleted {
                        agent_id,
                        task_id,
                        step_number,
                        steps_remaining,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentStepCompleted {
                            agent_id: agent_id.to_string(),
                            task_id: task_id.to_string(),
                            step_number: *step_number,
                            steps_remaining: *steps_remaining,
                        });
                    }
                    EventType::AgentHandoffRecorded {
                        handoff_id,
                        mission_id,
                        source_agent_id,
                        target_role,
                        reason,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentHandoffRecorded {
                            handoff_id: handoff_id.to_string(),
                            mission_id: mission_id.to_string(),
                            source_agent_id: source_agent_id.to_string(),
                            target_role: target_role.clone(),
                            reason: reason.clone(),
                        });
                    }
                    EventType::AgentCompleted {
                        agent_id,
                        mission_id,
                        summary,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentCompleted {
                            agent_id: agent_id.to_string(),
                            mission_id: mission_id.to_string(),
                            summary: summary.clone(),
                        });
                    }
                    EventType::AgentFailed {
                        agent_id,
                        mission_id,
                        error,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentFailed {
                            agent_id: agent_id.to_string(),
                            mission_id: mission_id.to_string(),
                            error: error.clone(),
                        });
                    }
                    EventType::AgentCancelled {
                        agent_id,
                        mission_id,
                        reason,
                    } => {
                        emit(&event_tx, InteractionEvent::AgentCancelled {
                            agent_id: agent_id.to_string(),
                            mission_id: mission_id.to_string(),
                            reason: reason.clone(),
                        });
                    }
                    EventType::JobStarted {
                        job_id,
                        mission_id,
                        job_type,
                    } => {
                        emit(&event_tx, InteractionEvent::JobStarted {
                            job_id: job_id.to_string(),
                            mission_id: mission_id.to_string(),
                            job_type: job_type.clone(),
                        });
                    }
                    EventType::JobCompleted {
                        job_id,
                        mission_id,
                        artifacts,
                    } => {
                        emit(&event_tx, InteractionEvent::JobCompleted {
                            job_id: job_id.to_string(),
                            mission_id: mission_id.to_string(),
                            artifacts: artifacts.iter().map(|a| a.to_string()).collect(),
                        });
                    }
                    EventType::JobFailed {
                        job_id,
                        mission_id,
                        error,
                    } => {
                        emit(&event_tx, InteractionEvent::JobFailed {
                            job_id: job_id.to_string(),
                            mission_id: mission_id.to_string(),
                            error: error.clone(),
                        });
                    }
                    EventType::CheckpointCreated {
                        checkpoint_id,
                        mission_id,
                        description,
                    } => {
                        emit(&event_tx, InteractionEvent::CheckpointCreated {
                            checkpoint_id: checkpoint_id.to_string(),
                            mission_id: mission_id.to_string(),
                            description: description.clone(),
                        });
                    }
                    EventType::CheckpointRestored {
                        checkpoint_id,
                        mission_id,
                    } => {
                        emit(&event_tx, InteractionEvent::CheckpointRestored {
                            checkpoint_id: checkpoint_id.to_string(),
                            mission_id: mission_id.to_string(),
                        });
                    }
                    EventType::RecoveryAttempted {
                        recovery_id,
                        mission_id,
                        strategy,
                        success,
                    } => {
                        emit(&event_tx, InteractionEvent::RecoveryAttempted {
                            recovery_id: recovery_id.clone(),
                            mission_id: mission_id.to_string(),
                            strategy: strategy.clone(),
                            success: *success,
                        });
                    }
                    EventType::TaskGraphMaterialized {
                        graph_id,
                        revision,
                        tasks,
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::TasksMaterialized {
                            graph_id: graph_id.to_string(),
                            revision: *revision,
                            tasks: tasks.clone(),
                        });
                    }
                    EventType::TaskStarted {
                        mission_id,
                        task_id,
                        agent_id,
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::TaskStarted {
                            mission_id: mission_id.to_string(),
                            task_id: task_id.to_string(),
                            agent_id: agent_id.to_string(),
                        });
                    }
                    EventType::TaskCompleted {
                        mission_id,
                        task_id,
                        result,
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::TaskCompleted {
                            mission_id: mission_id.to_string(),
                            task_id: task_id.to_string(),
                            result: result.clone(),
                        });
                    }
                    EventType::TaskFailed {
                        mission_id,
                        task_id,
                        error,
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::TaskFailed {
                            mission_id: mission_id.to_string(),
                            task_id: task_id.to_string(),
                            error: error.clone(),
                        });
                    }
                    EventType::TaskCancelled {
                        mission_id,
                        task_id,
                        reason,
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::TaskCancelled {
                            mission_id: mission_id.to_string(),
                            task_id: task_id.to_string(),
                            reason: reason.clone(),
                        });
                    }
                    EventType::WorkflowStarted { workflow_run_id, .. }
                    | EventType::WorkflowCompleted { workflow_run_id, .. }
                    | EventType::WorkflowFailed { workflow_run_id, .. }
                    | EventType::WorkflowCancelled { workflow_run_id, .. }
                    | EventType::WorkflowPaused { workflow_run_id, .. }
                    | EventType::WorkflowResumed { workflow_run_id, .. }
                    | EventType::WorkflowStepStarted { workflow_run_id, .. }
                    | EventType::WorkflowStepCompleted { workflow_run_id, .. }
                    | EventType::WorkflowStepFailed { workflow_run_id, .. }
                    | EventType::WorkflowStepBlocked { workflow_run_id, .. }
                    | EventType::WorkflowStepAwaitingApproval { workflow_run_id, .. }
                    | EventType::WorkflowStepAwaitingInput { workflow_run_id, .. }
                    | EventType::WorkflowStepSkipped { workflow_run_id, .. }
                    | EventType::WorkflowStepRetrying { workflow_run_id, .. } => {
                        if let Ok(snap) = runtime.get_workflow_snapshot(*workflow_run_id).await {
                            emit(&event_tx, InteractionEvent::WorkflowSnapshotUpdated {
                                snapshot: Box::new(snap),
                            });
                        }
                    }
                    _ => {}
                }
            }

            // 2. Supervise active background mission execution
            Some(res) = async {
                match active_execution.as_mut() {
                    Some(exec) => Some((&mut exec.join_handle).await),
                    None => std::future::pending().await,
                }
            } => {
                active_execution = None;
                if let Err(join_err) = res {
                    if join_err.is_panic() {
                        tracing::error!("Active mission execution task panicked: {:?}", join_err);
                        emit(&event_tx, InteractionEvent::Error {
                            message: "Mission execution task panicked".to_string(),
                        });
                    }
                }
            }

            // 3. Process actions originating from TUI
            Some(action) = action_rx.recv() => {
                let should_exit = dispatch_bridge_action(
                    action,
                    &mut runtime,
                    &workspace_root,
                    &session_repo,
                    &mut session,
                    &command_registry,
                    &mut cancel_token,
                    &mut active_execution,
                    &event_tx,
                ).await;
                if should_exit {
                    break;
                }
            }

            else => break,
        }
    }
}

async fn ensure_bridge_session<'a>(
    session: &'a mut Option<Session>,
    session_repo: &SqliteSessionRepository,
    workspace_root: &Path,
    event_tx: &UnboundedSender<InteractionEvent>,
) -> Result<&'a mut Session, String> {
    if session.is_none() {
        let new_s = session_repo
            .create_session(workspace_root)
            .await
            .map_err(|e| format!("Failed to create session: {e}"))?;
        emit(
            event_tx,
            InteractionEvent::SessionStarted {
                session_id: new_s.id,
            },
        );
        *session = Some(new_s);
    }
    Ok(session.as_mut().unwrap())
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_bridge_action(
    action: ApplicationAction,
    runtime: &mut Arc<AppRuntime>,
    workspace_root: &Path,
    session_repo: &SqliteSessionRepository,
    session: &mut Option<Session>,
    command_registry: &SlashCommandRegistry,
    cancel_token: &mut CancellationToken,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &UnboundedSender<InteractionEvent>,
) -> bool {
    match action {
        ApplicationAction::UserTextSubmitted(parsed) => {
            let session_ref = match ensure_bridge_session(
                session,
                session_repo,
                workspace_root,
                event_tx,
            )
            .await
            {
                Ok(s) => s,
                Err(err) => {
                    emit(event_tx, InteractionEvent::Error { message: err });
                    return false;
                }
            };
            // Runtime-layer ownership (§6–§7): conversational execution
            // (governed front door + AgentEngine continuation) lives in
            // `interaction::continuation`. The bridge only translates.
            crate::interaction::continuation::handle_user_text_submitted(
                runtime,
                workspace_root,
                session_repo,
                session_ref,
                &parsed,
                active_execution,
                event_tx,
            )
            .await;
        }

        ApplicationAction::SlashCommandSubmitted { command, args } => {
            let cmd_line = format!("/{} {}", command, args.join(" "));
            let ctx = CommandContext {
                workspace_root,
                session_id: session.as_ref().map(|s| s.id),
                active_mission_id: session.as_ref().and_then(|s| s.active_mission_id),
                pool: runtime.pool(),
                event_bus: runtime.event_bus(),
                tool_registry: Some(runtime.tool_registry().clone()),
                configured_model: runtime.config().active_model.clone(),
                // Runtime truth: the provider is reported only when its
                // authoritative status is Available — never from ambient
                // environment probing alone.
                configured_provider: if runtime.active_provider_status()
                    == crate::model::types::ProviderCapabilityStatus::Available
                {
                    runtime.config().active_provider.clone()
                } else {
                    "none".to_string()
                },
                active_profile: runtime
                    .config()
                    .active_profile
                    .clone()
                    .unwrap_or_else(|| "autonomous".to_string()),
                command_registry: Some(command_registry),
            };

            match command_registry.execute_line(&cmd_line, &ctx).await {
                Ok(CommandOutput::Info(txt)) => {
                    emit(event_tx, InteractionEvent::CommandOutput { text: txt });
                }
                Ok(CommandOutput::Error(err)) => {
                    emit(event_tx, InteractionEvent::Error { message: err });
                }
                Ok(CommandOutput::ApplicationAction(sub_act)) => {
                    return Box::pin(dispatch_bridge_action(
                        sub_act,
                        runtime,
                        workspace_root,
                        session_repo,
                        session,
                        command_registry,
                        cancel_token,
                        active_execution,
                        event_tx,
                    ))
                    .await;
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Command failed: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::UserCommandRequested { command, args } => {
            // Execute the global user command through the canonical runtime path.
            // This routes through PromptCatalog → PromptCompiler → ModelCaller
            // → PolicyGate → ApprovalCoordinator → ToolPipeline → Verification.
            emit(
                event_tx,
                InteractionEvent::ModelActivity {
                    text: format!("Executing user command '/{}'...", command),
                },
            );

            let session_ref = match ensure_bridge_session(
                session,
                session_repo,
                workspace_root,
                event_tx,
            )
            .await
            {
                Ok(s) => s,
                Err(err) => {
                    emit(event_tx, InteractionEvent::Error { message: err });
                    return false;
                }
            };

            if let Err(e) = runtime
                .execute_user_command(&command, args, session_ref.id)
                .await
            {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("User command '/{}' failed: {}", command, e),
                    },
                );
            }
        }

        ApplicationAction::DiffRequested => match runtime.get_git_diff().await {
            Ok(diff) => {
                emit(event_tx, InteractionEvent::CommandOutput { text: diff });
            }
            Err(e) => {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("Diff error: {e}"),
                    },
                );
            }
        },

        ApplicationAction::CommitRequested { message } => {
            let active_mission_id = session.as_ref().and_then(|s| s.active_mission_id);
            match runtime
                .commit_changes(active_mission_id, message.as_deref())
                .await
            {
                Ok(summary) => {
                    emit(event_tx, InteractionEvent::CommandOutput { text: summary });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Commit failed: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::StatusRequested => {
            let ctx = CommandContext {
                workspace_root,
                session_id: session.as_ref().map(|s| s.id),
                active_mission_id: session.as_ref().and_then(|s| s.active_mission_id),
                pool: runtime.pool(),
                event_bus: runtime.event_bus(),
                tool_registry: Some(runtime.tool_registry().clone()),
                configured_model: runtime.config().active_model.clone(),
                // Runtime truth: the provider is reported only when its
                // authoritative status is Available — never from ambient
                // environment probing alone.
                configured_provider: if runtime.active_provider_status()
                    == crate::model::types::ProviderCapabilityStatus::Available
                {
                    runtime.config().active_provider.clone()
                } else {
                    "none".to_string()
                },
                active_profile: runtime
                    .config()
                    .active_profile
                    .clone()
                    .unwrap_or_else(|| "autonomous".to_string()),
                command_registry: Some(command_registry),
            };
            if let Ok(CommandOutput::Info(info)) =
                command_registry.execute_line("/status", &ctx).await
            {
                emit(event_tx, InteractionEvent::CommandOutput { text: info });
            }
        }

        ApplicationAction::ClearRequested => {
            emit(
                event_tx,
                InteractionEvent::CommandOutput {
                    text: "Console cleared.".to_string(),
                },
            );
        }

        ApplicationAction::ClearSessionRequested => {
            if let Some(s) = session.as_ref() {
                let _ = session_repo.clear_conversation(s.id).await;
                emit(
                    event_tx,
                    InteractionEvent::CommandOutput {
                        text: "Conversation cleared.".to_string(),
                    },
                );
            } else {
                emit(
                    event_tx,
                    InteractionEvent::CommandOutput {
                        text: "No active session to clear.".to_string(),
                    },
                );
            }
        }

        ApplicationAction::ModelChangeRequested { model } => {
            match runtime.config().with_session_model(&model) {
                Ok(new_cfg) => {
                    let new_runtime = (**runtime).clone().with_config(Arc::new(new_cfg));
                    *runtime = Arc::new(new_runtime);
                    let text = format!("Active model switched to '{}' for this session.", model);
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: text.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(
                        event_tx,
                        InteractionEvent::ConfigurationUpdated {
                            model: runtime.config().active_model.clone(),
                            provider: runtime.config().active_provider.clone(),
                            profile: runtime.config().active_profile.clone(),
                        },
                    );
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to switch model: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::ProviderChangeRequested { provider } => {
            match runtime.config().with_session_provider(&provider) {
                Ok(new_cfg) => {
                    let new_runtime = (**runtime).clone().with_config(Arc::new(new_cfg));
                    *runtime = Arc::new(new_runtime);
                    let text = format!(
                        "Active provider switched to '{}' for this session.",
                        provider
                    );
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: text.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(
                        event_tx,
                        InteractionEvent::ConfigurationUpdated {
                            model: runtime.config().active_model.clone(),
                            provider: runtime.config().active_provider.clone(),
                            profile: runtime.config().active_profile.clone(),
                        },
                    );
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to switch provider: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::ProfileChangeRequested { profile } => {
            match runtime.config().with_session_profile(&profile) {
                Ok(new_cfg) => {
                    let new_runtime = (**runtime).clone().with_config(Arc::new(new_cfg));
                    *runtime = Arc::new(new_runtime);
                    let text = format!("Autonomy profile set to '{}' for this session.", profile);
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: text.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(
                        event_tx,
                        InteractionEvent::ConfigurationUpdated {
                            model: runtime.config().active_model.clone(),
                            provider: runtime.config().active_provider.clone(),
                            profile: runtime.config().active_profile.clone(),
                        },
                    );
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to switch profile: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::MissionPauseRequested { mission_id } => {
            if let Ok(mid) = mission_id.parse::<MissionId>() {
                match runtime.pause_mission(mid, "Operator request via TUI").await {
                    Ok(()) => {
                        emit(
                            event_tx,
                            InteractionEvent::CommandOutput {
                                text: format!("Mission '{mission_id}' paused."),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to pause mission '{mission_id}': {e}"),
                            },
                        );
                    }
                }
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("Invalid mission id: {mission_id}"),
                    },
                );
            }
        }

        ApplicationAction::MissionResumeRequested { mission_id } => {
            if let Ok(mid) = mission_id.parse::<MissionId>() {
                match runtime.resume_mission(mid).await {
                    Ok(()) => {
                        emit(
                            event_tx,
                            InteractionEvent::CommandOutput {
                                text: format!("Mission '{mission_id}' resumed."),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to resume mission '{mission_id}': {e}"),
                            },
                        );
                    }
                }
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("Invalid mission id: {mission_id}"),
                    },
                );
            }
        }

        ApplicationAction::ConfigOverrideRequested { key, value } => {
            let parsed_val = serde_json::from_str::<serde_json::Value>(&value)
                .unwrap_or_else(|_| serde_json::Value::String(value.clone()));
            match runtime.config().with_session_override(&key, parsed_val) {
                Ok(new_cfg) => {
                    let new_runtime = (**runtime).clone().with_config(Arc::new(new_cfg));
                    *runtime = Arc::new(new_runtime);
                    let text = format!("Session override applied: {} = {}", key, value);
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: text.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply configuration override: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::SettingsRequested { category } => {
            let text = match category {
                Some(c) => format!("Opening settings ({c}) — canonical configuration editor."),
                None => "Opening settings — canonical configuration editor.".to_string(),
            };
            emit(event_tx, InteractionEvent::CommandOutput { text });
        }

        ApplicationAction::SessionResumeRequested { session_id } => {
            if let Ok(uuid) = session_id.parse::<uuid::Uuid>() {
                let sid = SessionId::from(uuid);
                match session_repo.get_session(sid).await {
                    Ok(Some(s)) => {
                        let sess_ws =
                            crate::init::instance::canonicalize_workspace_root(&s.workspace_root);
                        let runtime_ws = crate::init::instance::canonicalize_workspace_root(
                            runtime.workspace_root(),
                        );
                        if sess_ws != runtime_ws {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Refusing to resume session '{session_id}': session workspace '{}' != runtime workspace '{}'; rebind before execution.",
                                        s.workspace_root.display(),
                                        runtime.workspace_root().display()
                                    ),
                                },
                            );
                        } else {
                            let resumed_id = s.id;
                            emit(
                                event_tx,
                                InteractionEvent::SessionResumed {
                                    session_id: resumed_id,
                                },
                            );
                            let events = hydrate_session_state(runtime, session_repo, &s).await;
                            for ev in events {
                                emit(event_tx, ev);
                            }
                            *session = Some(s);
                        }
                    }
                    Ok(None) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Session '{session_id}' not found."),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to resume session: {e}"),
                            },
                        );
                    }
                }
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("Invalid session ID: '{session_id}'"),
                    },
                );
            }
        }

        ApplicationAction::ApprovalDecision {
            request_id,
            decision,
        } => {
            let action = match decision {
                ApprovalDecision::ApproveOnce => crate::policy::approval::ApprovalAction::AllowOnce,
                ApprovalDecision::ApproveAlways => {
                    crate::policy::approval::ApprovalAction::AllowForSession
                }
                ApprovalDecision::Reject => crate::policy::approval::ApprovalAction::Deny {
                    reason: "operator rejected".to_string(),
                },
                ApprovalDecision::Dismiss => crate::policy::approval::ApprovalAction::Deny {
                    reason: "operator dismissed".to_string(),
                },
                ApprovalDecision::Edit => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: "Direct parameter editing of tool proposals is not supported by the runtime.".to_string(),
                        },
                    );
                    return false;
                }
            };
            let approved = action.is_allowed();
            let req_uuid = match uuid::Uuid::parse_str(&request_id) {
                Ok(u) => u,
                Err(err) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!(
                                "Cannot resolve approval: invalid request ID '{request_id}': {err}"
                            ),
                        },
                    );
                    return false;
                }
            };

            match runtime
                .approval_coordinator()
                .resolve_request(req_uuid.into(), action, "operator")
                .await
            {
                Ok(()) => {
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::ApprovalMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                request_id: request_id.clone(),
                                prompt: "Approval resolved by operator".to_string(),
                                decision: Some(format!("{:?}", decision)),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }

                    emit(
                        event_tx,
                        InteractionEvent::ApprovalResolved {
                            request_id,
                            approved,
                        },
                    );
                }
                Err(err) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to resolve approval '{request_id}': {err}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::CancelRequested => {
            cancel_token.cancel();
            *cancel_token = CancellationToken::new();

            if let Some(exec) = active_execution.take() {
                exec.cancel_token.cancel();
                exec.join_handle.abort();
            }

            if let Some(s) = session.as_ref() {
                if let Some(mid) = s.active_mission_id {
                    let _ = runtime
                        .cancel_mission(mid, "Operator cancelled via TUI session")
                        .await;
                }

                // transition active non-terminal pre-execution lifecycle state to cancelled
                let coordinator = runtime.create_pre_execution_coordinator();
                let sid_str = s.id.to_string();
                if let Ok(Some(ls)) = coordinator
                    .lifecycle_repo()
                    .load_lifecycle_state(&sid_str)
                    .await
                {
                    if !ls.stage.is_terminal() {
                        let _ = coordinator
                            .lifecycle_repo()
                            .save_validated_transition(
                                &sid_str,
                                ls.stage,
                                crate::state_machine::lifecycle::LifecycleEvent::Cancel,
                            )
                            .await;
                    }
                }

                if let Ok(seq) = session_repo.next_sequence(s.id).await {
                    let turn = ConversationTurn::SystemMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: "Operation cancelled.".to_string(),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = session_repo.append_turn(s.id, &turn).await;
                }
            }

            emit(
                event_tx,
                InteractionEvent::CommandOutput {
                    text: "Operation cancelled.".to_string(),
                },
            );
        }

        ApplicationAction::ExitRequested => {
            if let Some(s) = session.as_ref() {
                let _ = session_repo.update_status(s.id, SessionState::Closed).await;
            }
            return true;
        }

        // Governed lifecycle actions. Every mutating governance
        // decision routes through the canonical PreExecutionCoordinator; the
        // TUI requests, the runtime decides. Failures are surfaced explicitly.
        ApplicationAction::IntentInitRequested { .. }
        | ApplicationAction::QuestionAnswerSubmitted { .. }
        | ApplicationAction::PlanEditRequested { .. }
        | ApplicationAction::PlanRevisionRequested { .. }
        | ApplicationAction::PlanRegenerateRequested { .. }
        | ApplicationAction::PlanAcceptRequested { .. }
        | ApplicationAction::PlanRejectRequested { .. }
        | ApplicationAction::TaskEditRequested { .. }
        | ApplicationAction::TaskAddRequested { .. }
        | ApplicationAction::TaskRemoveRequested { .. }
        | ApplicationAction::TaskRegenerateRequested { .. }
        | ApplicationAction::TasksAcceptRequested { .. }
        | ApplicationAction::ExecutionAuthorizationSubmitted { .. } => {
            let session_ref = match ensure_bridge_session(
                session,
                session_repo,
                workspace_root,
                event_tx,
            )
            .await
            {
                Ok(s) => s,
                Err(err) => {
                    emit(event_tx, InteractionEvent::Error { message: err });
                    return false;
                }
            };
            crate::interaction::continuation::handle_lifecycle_action(
                action,
                runtime,
                session_repo,
                session_ref,
                active_execution,
                event_tx,
            )
            .await;
        }

        // Genesis and Workflow route to the same canonical
        // runtime owners as the CLI path. The TUI requests; the runtime
        // decides. Nothing here synthesizes success.
        ApplicationAction::GenesisRequested { prompt, mode } => {
            let gen_mode = match mode.as_deref() {
                Some("greenfield") => crate::workflow::genesis::GenesisMode::Greenfield,
                Some("brownfield") => crate::workflow::genesis::GenesisMode::Brownfield,
                _ => crate::workflow::genesis::GenesisMode::AutoDetect,
            };
            let req = crate::workflow::genesis::GenesisRequest::new(prompt.clone(), workspace_root)
                .with_mode(gen_mode);
            match runtime.run_genesis(&req).await {
                Ok(outcome) => {
                    let text = format!(
                        "Project Genesis planning complete for '{}'. Roadmap lowered to workflow '{}' with {} phases. Registered artifacts: {}.",
                        outcome.charter.project_name,
                        outcome.workflow_definition.id,
                        outcome.planning.roadmap.phases.len(),
                        outcome.registered_artifacts.len(),
                    );
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: text.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    let message = format!("Genesis error: {e}");
                    if let Some(s) = session.as_ref() {
                        if let Ok(seq) = session_repo.next_sequence(s.id).await {
                            let turn = ConversationTurn::SystemMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: message.clone(),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = session_repo.append_turn(s.id, &turn).await;
                        }
                    }
                    emit(event_tx, InteractionEvent::Error { message });
                }
            }
        }

        ApplicationAction::GenesisStateRequested => {
            match runtime.get_genesis_state(".planning").await {
                Ok(Some(state)) => {
                    emit(
                        event_tx,
                        InteractionEvent::CommandOutput {
                            text: format!(
                                "Planning state: lifecycle={:?} status={} phase={:?}",
                                state.lifecycle_state, state.approval_status, state.current_phase
                            ),
                        },
                    );
                }
                Ok(None) => {
                    emit(
                        event_tx,
                        InteractionEvent::CommandOutput {
                            text: "No Project Genesis state found in .planning/STATE.md"
                                .to_string(),
                        },
                    );
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Error inspecting Genesis state: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::WorkflowResumeRequested { run_id } => {
            match runtime.handle_workflow_resume(&run_id).await {
                Ok(report) => {
                    emit_workflow_snapshot_if_known(runtime, &run_id, event_tx).await;
                    emit(event_tx, InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Workflow resume failed for run {run_id}: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::WorkflowApprovalSubmitted {
            run_id,
            step_key,
            approved,
            reason,
        } => {
            match runtime
                .handle_workflow_approval(&run_id, &step_key, approved, reason)
                .await
            {
                Ok(report) => {
                    emit_workflow_snapshot_if_known(runtime, &run_id, event_tx).await;
                    emit(event_tx, InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!(
                                "Workflow approval failed for run {run_id}, step {step_key}: {e}"
                            ),
                        },
                    );
                }
            }
        }

        ApplicationAction::WorkflowPauseRequested { run_id, reason } => {
            match runtime.pause_workflow_run(&run_id, &reason).await {
                Ok(report) => {
                    emit_workflow_snapshot_if_known(runtime, &run_id, event_tx).await;
                    emit(event_tx, InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Workflow pause failed for run {run_id}: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::WorkflowCancelRequested { run_id, reason } => {
            match runtime.cancel_workflow_run(&run_id, &reason).await {
                Ok(report) => {
                    emit_workflow_snapshot_if_known(runtime, &run_id, event_tx).await;
                    emit(event_tx, InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Workflow cancel failed for run {run_id}: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::WorkflowInspectRequested { run_id } => {
            match runtime.inspect_workflow_run(&run_id).await {
                Ok(report) => {
                    emit_workflow_snapshot_if_known(runtime, &run_id, event_tx).await;
                    emit(event_tx, InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Workflow inspect failed for run {run_id}: {e}"),
                        },
                    );
                }
            }
        }

        ApplicationAction::MentionResolved(reference) => {
            // Read-only inspection: mentions already resolve inside the
            // governed front door; a standalone resolve only reports the
            // recorded resolution truthfully and mutates nothing.
            let text = match &reference.status {
                crate::interaction::mentions::ResolutionStatus::Resolved => {
                    format!(
                        "Mention '{}' resolved to workspace path '{}'.",
                        reference.original_text,
                        reference.workspace_relative_path.display()
                    )
                }
                other => {
                    format!(
                        "Mention '{}' did not resolve: {other:?}.",
                        reference.original_text
                    )
                }
            };
            emit(event_tx, InteractionEvent::CommandOutput { text });
        }
    }

    false
}

async fn emit_workflow_snapshot_if_known(
    runtime: &AppRuntime,
    run_id_str: &str,
    event_tx: &UnboundedSender<InteractionEvent>,
) {
    if let Ok(run_id) = run_id_str.parse::<crate::ids::WorkflowRunId>() {
        if let Ok(snap) = runtime.get_workflow_snapshot(run_id).await {
            emit(
                event_tx,
                InteractionEvent::WorkflowSnapshotUpdated {
                    snapshot: Box::new(snap),
                },
            );
        }
    }
}
