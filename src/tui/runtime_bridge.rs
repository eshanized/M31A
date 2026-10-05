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

use crate::agent::engine::AgentTurnOutcome;
use crate::error::M31AError;
use crate::events::bus::{EventBus, EventFilter};
use crate::events::types::EventType;
use crate::ids::{MissionId, SessionId};
use crate::interaction::action::ApplicationAction;
use crate::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use crate::interaction::events::InteractionEvent;
use crate::interaction::mentions::{MentionParser, ParsedUserMessage};
use crate::interaction::session::{
    ConversationTurn, Session, SessionState, SqliteSessionRepository,
};
use crate::persistence::sqlite::repositories::SqliteMissionRepository;
use crate::planning::review::{PlanRevision, PreExecutionResponse, TaskRevision};
use crate::runtime::AppRuntime;
use crate::state::Mission;
use crate::state_machine::lifecycle::LifecycleStage;
use crate::tui::approval::ApprovalDecision;

/// Supervised background mission execution handle running concurrently with the bridge.
struct ActiveExecution {
    #[allow(dead_code)]
    mission_id: MissionId,
    join_handle: tokio::task::JoinHandle<()>,
    cancel_token: CancellationToken,
}

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

    /// Take the event receiver for passing to `TuiApp`.
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

        // 2. Initialize or resume durable session
        let session = if let Some(sid) = resume_session_id {
            if let Some(s) = session_repo.get_session(sid).await? {
                s
            } else {
                session_repo.create_session(&workspace_root).await?
            }
        } else {
            let all_sessions = session_repo.list_sessions().await.unwrap_or_default();
            if let Some(active) = all_sessions
                .into_iter()
                .find(|s| s.workspace_root == workspace_root && s.status == SessionState::Active)
            {
                active
            } else {
                session_repo.create_session(&workspace_root).await?
            }
        };

        let active_sid = session.id;
        let was_resume = resume_session_id.is_some() || session.status == SessionState::Active;

        // Emit initial session event
        emit(
            &event_tx,
            InteractionEvent::SessionStarted {
                session_id: active_sid,
            },
        );
        if was_resume {
            emit(
                &event_tx,
                InteractionEvent::SessionResumed {
                    session_id: active_sid,
                },
            );
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

        // 3. Hydrate persisted session truth into interaction events
        for ev in hydrate_session_state(&runtime, &session_repo, &session).await {
            emit(&event_tx, ev);
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
            session_id: Some(active_sid),
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
        match coordinator.resume_session(&sid_str).await {
            Ok(resp) => response_to_hydration_events(resp, &mut out),
            Err(e) => {
                out.push(InteractionEvent::Error {
                    message: format!("Session hydration: lifecycle resume failed: {e}"),
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
            out.push(InteractionEvent::ModelUsageUpdated {
                invocation_id: Some(r.id.to_string()),
                prompt_tokens: r.prompt_tokens as u64,
                completion_tokens: r.completion_tokens as u64,
                total_tokens: r.total_tokens as u64,
                cost_cents: None,
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
            out.push(InteractionEvent::PlanForReview {
                session_id,
                revision: revision.revision,
                plan_id: revision.plan_id.clone(),
                objective: revision.content.objective.clone(),
                task_count: revision.content.tasks.len(),
                content_hash: Some(content_hash),
            });
        }
        PreExecutionResponse::TasksForReview {
            session_id,
            revision,
        } => {
            let content_hash = TaskRevision::compute_tasks_hash(&revision.tasks);
            out.push(InteractionEvent::TasksForReview {
                session_id,
                plan_revision: revision.plan_revision,
                task_revision: revision.revision,
                task_count: revision.tasks.len(),
                content_hash: Some(content_hash),
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
/// Upper bound for one natural-language request through the governed front
/// door plus the legacy agent-engine continuation.
///
/// This is NOT success simulation: on expiry the bridge emits an explicit
/// `Error` event (a valid terminal outcome) instead of leaving the TUI in
/// a silent Thinking state forever. Genuine long model calls complete well
/// inside the bound; only hung calls hit it, and they fail explicitly.
const USER_REQUEST_TIMEOUT_SECS: u64 = 300;

/// Emit an interaction event toward the TUI, loudly.
///
/// A failed send means the TUI is gone; that is logged, never silently
/// ignored, so abandoned requests are observable in diagnostics.
fn emit(event_tx: &UnboundedSender<InteractionEvent>, event: InteractionEvent) -> bool {
    if event_tx.send(event).is_err() {
        tracing::warn!("TUI bridge: interaction receiver dropped; event abandoned");
        false
    } else {
        true
    }
}

/// The asynchronous background worker driving execution and event routing.
async fn run_bridge_worker(
    mut runtime: Arc<AppRuntime>,
    workspace_root: PathBuf,
    session_repo: SqliteSessionRepository,
    mut session: Session,
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
                    EventType::OperatorEscalationRequested { request_id, reason, .. } => {
                        emit(&event_tx, InteractionEvent::ApprovalRequested {
                            request_id: request_id.clone(),
                            tool_name: "".to_string(),
                            details: reason.clone(),
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
                    EventType::VerificationCompleted { passed, evidence, .. } => {
                        if *passed {
                            emit(&event_tx, InteractionEvent::VerificationPassed {
                                summary: evidence.clone(),
                            });
                        } else {
                            emit(&event_tx, InteractionEvent::VerificationFailed {
                                summary: evidence.clone(),
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
                        ..
                    } => {
                        emit(&event_tx, InteractionEvent::ModelUsageUpdated {
                            invocation_id: invocation_id.map(|u| u.to_string()),
                            prompt_tokens: usage.prompt_tokens as u64,
                            completion_tokens: usage.completion_tokens as u64,
                            total_tokens: usage.total_tokens as u64,
                            cost_cents: None,
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

#[allow(clippy::too_many_arguments)]
async fn dispatch_bridge_action(
    action: ApplicationAction,
    runtime: &mut Arc<AppRuntime>,
    workspace_root: &Path,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    command_registry: &SlashCommandRegistry,
    cancel_token: &mut CancellationToken,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &UnboundedSender<InteractionEvent>,
) -> bool {
    match action {
        ApplicationAction::UserTextSubmitted(parsed) => {
            // Request correlation: the TUI stamps one id per submission; the
            // whole request below is span-scoped by it so submission,
            // bridge, coordinator/agent invocation, and emitted events are
            // traceable end to end in diagnostics.
            let request_id = parsed
                .request_id
                .clone()
                .unwrap_or_else(|| "unstamped".to_string());
            let span = tracing::info_span!("tui_user_request", %request_id);
            let _guard = span.enter();
            tracing::info!("handling user text request");

            // Inject @mention context
            let mention_ctx =
                MentionParser::inject_mention_context(workspace_root, &parsed.mentions);
            let full_prompt = format!("{}{}", parsed.normalized_prompt(), mention_ctx);

            // Persist user turn in SQLite
            if let Ok(seq) = session_repo.next_sequence(session.id).await {
                let turn = ConversationTurn::UserMessage {
                    id: uuid::Uuid::now_v7(),
                    sequence: seq,
                    content: full_prompt,
                    raw_text: parsed.raw_text.clone(),
                    mentions: parsed.mentions.clone(),
                    created_at: chrono::Utc::now(),
                };
                let _ = session_repo.append_turn(session.id, &turn).await;
            }

            // No generic ModelActivity is emitted here. Activity state is
            // owned by the precise outcome below (governed lifecycle event
            // or AgentEngine stream), never announced before routing knows
            // what the operation actually is.
            //
            // Bounded explicitly: a hung model/provider call must surface an
            // explicit Error, never indefinite Thinking silence. The timeout
            // manufactures no success and clears no UI — it reports.
            let outcome = tokio::time::timeout(
                std::time::Duration::from_secs(USER_REQUEST_TIMEOUT_SECS),
                async {
                    // INVARIANT F — governed front door (same rule as the
                    // CLI runner): new intents and discovery answers route through the
                    // PreExecutionCoordinator; free text in review/authorization stages
                    // is rejected fail-closed; only executing/terminal continuations
                    // reach the legacy AgentEngine below.
                    if try_governed_front_door(
                        runtime,
                        workspace_root,
                        session,
                        &parsed,
                        active_execution,
                        event_tx,
                    )
                    .await
                    {
                        return;
                    }

                    let (chunk_tx, mut chunk_rx) = unbounded_channel();
                    let mut engine = runtime
                        .create_agent_engine(session.id)
                        .with_stream_sender(chunk_tx);
                    let _ = engine.load_session_state().await;

                    emit(
                        event_tx,
                        InteractionEvent::ModelActivity {
                            text: "Reasoning and executing via AgentEngine...".to_string(),
                        },
                    );

                    let stream_event_tx = event_tx.clone();
                    let stream_forwarder = tokio::spawn(async move {
                        let mut message_id = uuid::Uuid::now_v7().to_string();
                        let mut stream_started = false;
                        while let Some(chunk) = chunk_rx.recv().await {
                            match chunk {
                                crate::model::types::StreamChunk::TextDelta(delta) => {
                                    if !stream_started {
                                        emit(
                                            &stream_event_tx,
                                            InteractionEvent::AssistantStarted {
                                                message_id: message_id.clone(),
                                            },
                                        );
                                        stream_started = true;
                                    }
                                    emit(
                                        &stream_event_tx,
                                        InteractionEvent::AssistantDelta {
                                            message_id: message_id.clone(),
                                            delta,
                                        },
                                    );
                                }
                                crate::model::types::StreamChunk::ToolCallDelta {
                                    name, ..
                                } => {
                                    if let Some(tool_name) = name {
                                        emit(
                                            &stream_event_tx,
                                            InteractionEvent::ModelActivity {
                                                text: format!("Preparing tool `{tool_name}`..."),
                                            },
                                        );
                                    }
                                }
                                crate::model::types::StreamChunk::UsageUpdate(usage) => {
                                    emit(
                                        &stream_event_tx,
                                        InteractionEvent::ModelUsageUpdated {
                                            invocation_id: None,
                                            prompt_tokens: usage.prompt_tokens as u64,
                                            completion_tokens: usage.completion_tokens as u64,
                                            total_tokens: usage.total_tokens as u64,
                                            cost_cents: None,
                                        },
                                    );
                                }
                                crate::model::types::StreamChunk::FinishReason(_) => {
                                    if stream_started {
                                        emit(
                                            &stream_event_tx,
                                            InteractionEvent::AssistantFinished {
                                                message_id: message_id.clone(),
                                            },
                                        );
                                        message_id = uuid::Uuid::now_v7().to_string();
                                        stream_started = false;
                                    }
                                }
                            }
                        }
                        if stream_started {
                            emit(
                                &stream_event_tx,
                                InteractionEvent::AssistantFinished { message_id },
                            );
                        }
                    });

                    let final_state_res = engine
                        .run_continuous(|outcome| {
                            match outcome {
                                AgentTurnOutcome::AssistantCommentary { content } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::AssistantOutput {
                                            text: content.clone(),
                                        },
                                    );
                                }
                                AgentTurnOutcome::AssistantText { content } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::AssistantOutput {
                                            text: content.clone(),
                                        },
                                    );
                                }
                                AgentTurnOutcome::ToolResults { results } => {
                                    for r in results {
                                        emit(
                                            event_tx,
                                            InteractionEvent::ToolCompleted {
                                                call_id: r.call_id.clone(),
                                                tool_name: r.tool_name.clone(),
                                                success: r.success,
                                                output_preview: r
                                                    .output
                                                    .chars()
                                                    .take(200)
                                                    .collect(),
                                            },
                                        );
                                    }
                                }
                                AgentTurnOutcome::WaitingForUser { question, .. } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::AssistantOutput {
                                            text: format!("[Question] {question}"),
                                        },
                                    );
                                }
                                AgentTurnOutcome::WaitingForApproval {
                                    request_id,
                                    tool_name,
                                    parameters,
                                } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::ApprovalRequested {
                                            request_id: request_id.clone(),
                                            tool_name: tool_name.clone(),
                                            details: parameters.to_string(),
                                        },
                                    );
                                }
                                AgentTurnOutcome::Completed { summary } => {
                                    // §39: an assistant turn completing is NOT mission
                                    // completion. Mission completion originates only
                                    // from the verification-gated completion path
                                    // (MissionCompleted event). Report the turn
                                    // truthfully as assistant output.
                                    emit(
                                        event_tx,
                                        InteractionEvent::AssistantOutput {
                                            text: summary.clone(),
                                        },
                                    );
                                }
                                AgentTurnOutcome::Failed { error } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::VerificationFailed {
                                            summary: error.clone(),
                                        },
                                    );
                                    emit(
                                        event_tx,
                                        InteractionEvent::Error {
                                            message: error.clone(),
                                        },
                                    );
                                }
                                AgentTurnOutcome::Cancelled { reason } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::Error {
                                            message: format!("Cancelled: {reason}"),
                                        },
                                    );
                                }
                                AgentTurnOutcome::BudgetExhausted { reason } => {
                                    emit(
                                        event_tx,
                                        InteractionEvent::Error {
                                            message: format!("Turn budget exhausted: {reason}"),
                                        },
                                    );
                                }
                            }
                            Ok(())
                        })
                        .await;

                    drop(engine);
                    let _ = stream_forwarder.await;

                    if let Err(e) = final_state_res {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("AgentEngine execution error: {e}"),
                            },
                        );
                    }
                },
            )
            .await;
            if outcome.is_err() {
                tracing::error!(%request_id, "user text request timed out");
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!(
                            "Request {request_id} timed out after {}s without a runtime outcome; the operation was abandoned and nothing was executed on your behalf.",
                            USER_REQUEST_TIMEOUT_SECS
                        ),
                    },
                );
            }
        }

        ApplicationAction::SlashCommandSubmitted { command, args } => {
            let cmd_line = format!("/{} {}", command, args.join(" "));
            let ctx = CommandContext {
                workspace_root,
                session_id: Some(session.id),
                active_mission_id: session.active_mission_id,
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

            if let Err(e) = runtime
                .execute_user_command(&command, args, session.id)
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
            match runtime
                .commit_changes(session.active_mission_id, message.as_deref())
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
                session_id: Some(session.id),
                active_mission_id: session.active_mission_id,
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
            let _ = session_repo.clear_conversation(session.id).await;
            emit(
                event_tx,
                InteractionEvent::CommandOutput {
                    text: "Conversation cleared.".to_string(),
                },
            );
        }

        ApplicationAction::ModelChangeRequested { model } => {
            match runtime.config().with_session_model(&model) {
                Ok(new_cfg) => {
                    let new_runtime = (**runtime).clone().with_config(Arc::new(new_cfg));
                    *runtime = Arc::new(new_runtime);
                    let text = format!("Active model switched to '{}' for this session.", model);
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: text.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
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
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: text.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
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
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: text.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
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
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: text.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
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

        ApplicationAction::SessionResumeRequested { session_id } => {
            if let Ok(uuid) = session_id.parse::<uuid::Uuid>() {
                let sid = SessionId::from(uuid);
                match session_repo.get_session(sid).await {
                    Ok(Some(s)) => {
                        *session = s;
                        emit(
                            event_tx,
                            InteractionEvent::CommandOutput {
                                text: format!("Resumed session: {}", session.id),
                            },
                        );
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
                ApprovalDecision::Edit => crate::policy::approval::ApprovalAction::Deny {
                    reason: "operator requested edit".to_string(),
                },
            };
            let approved = action.is_allowed();
            if let Ok(req_uuid) = uuid::Uuid::parse_str(&request_id) {
                let _ = runtime
                    .approval_coordinator()
                    .resolve_request(req_uuid.into(), action, "operator")
                    .await;

                if let Ok(seq) = session_repo.next_sequence(session.id).await {
                    let turn = ConversationTurn::ApprovalMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        request_id: request_id.clone(),
                        prompt: "Approval resolved by operator".to_string(),
                        decision: Some(format!("{:?}", decision)),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = session_repo.append_turn(session.id, &turn).await;
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

        ApplicationAction::CancelRequested => {
            cancel_token.cancel();
            *cancel_token = CancellationToken::new();

            if let Some(exec) = active_execution.take() {
                exec.cancel_token.cancel();
                exec.join_handle.abort();
            }

            if let Some(mid) = session.active_mission_id {
                let _ = runtime
                    .cancel_mission(mid, "Operator cancelled via TUI session")
                    .await;
            }

            if let Ok(seq) = session_repo.next_sequence(session.id).await {
                let turn = ConversationTurn::SystemMessage {
                    id: uuid::Uuid::now_v7(),
                    sequence: seq,
                    content: "Operation cancelled.".to_string(),
                    created_at: chrono::Utc::now(),
                };
                let _ = session_repo.append_turn(session.id, &turn).await;
            }

            emit(
                event_tx,
                InteractionEvent::CommandOutput {
                    text: "Operation cancelled.".to_string(),
                },
            );
        }

        ApplicationAction::ExitRequested => {
            let _ = session_repo
                .update_status(session.id, SessionState::Closed)
                .await;
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
            handle_lifecycle_action(
                action,
                runtime,
                session_repo,
                session,
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
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: text.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
                    }
                    emit(event_tx, InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    let message = format!("Genesis error: {e}");
                    if let Ok(seq) = session_repo.next_sequence(session.id).await {
                        let turn = ConversationTurn::SystemMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: message.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = session_repo.append_turn(session.id, &turn).await;
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

/// Governed front door for TUI free-text input (INVARIANT F).
///
/// Mirrors the CLI runner rule: new intents enter the PreExecution lifecycle,
/// discovery answers route to the pending question, free text in
/// review/authorization stages is rejected with explicit guidance, and only
/// executing/terminal continuations fall through to the legacy AgentEngine.
///
/// Returns true when the input was consumed by the governed path.
async fn try_governed_front_door(
    runtime: &mut Arc<AppRuntime>,
    workspace_root: &Path,
    session: &mut Session,
    parsed: &ParsedUserMessage,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &UnboundedSender<InteractionEvent>,
) -> bool {
    let mention_ctx = MentionParser::inject_mention_context(workspace_root, &parsed.mentions);
    let full_prompt = format!("{}{}", parsed.normalized_prompt(), mention_ctx);

    let coordinator = runtime.create_pre_execution_coordinator();
    let sid_str = session.id.to_string();
    let lifecycle_state = coordinator
        .lifecycle_repo()
        .load_lifecycle_state(&sid_str)
        .await
        .unwrap_or(None);
    let governed_stage = lifecycle_state.as_ref().map(|ls| ls.stage);

    match governed_stage {
        // No active lifecycle: start the governed PreExecution lifecycle.
        None => {
            match coordinator
                .init_intent(&sid_str, &full_prompt, "operator")
                .await
            {
                Ok(resp) => {
                    emit_lifecycle_response(resp, runtime, session, active_execution, event_tx)
                        .await;
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Error starting governed lifecycle: {e}"),
                        },
                    );
                }
            }
            true
        }
        // Discovery stages: free text answers the pending blocking question.
        Some(LifecycleStage::AwaitingInformation) | Some(LifecycleStage::IntentActive) => {
            let questions = coordinator
                .lifecycle_repo()
                .load_discovery_questions(&sid_str)
                .await
                .unwrap_or_default();
            let pending: Vec<_> = questions
                .into_iter()
                .filter(|q| q.answer.is_none())
                .collect();
            if let Some(q) = pending.first() {
                match coordinator
                    .submit_answer(&sid_str, &q.question_id, &full_prompt, "operator")
                    .await
                {
                    Ok(resp) => {
                        emit_lifecycle_response(resp, runtime, session, active_execution, event_tx)
                            .await;
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Error submitting discovery answer: {e}"),
                            },
                        );
                    }
                }
                true
            } else {
                // No pending questions: legacy conversational continuation.
                false
            }
        }
        // Review and authorization stages: free text cannot advance governance.
        // Fail closed with explicit guidance instead of executing.
        Some(
            LifecycleStage::PlanDraft
            | LifecycleStage::PlanReview
            | LifecycleStage::PlanRevision
            | LifecycleStage::PlanAccepted
            | LifecycleStage::TasksDraft
            | LifecycleStage::TasksReview
            | LifecycleStage::TasksRevision
            | LifecycleStage::TasksAccepted
            | LifecycleStage::ExecutionAwaitingAuthorization
            | LifecycleStage::ExecutionAuthorized,
        ) => {
            let stage = governed_stage.expect("stage matched above");
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!(
                        "Governed lifecycle is in stage '{stage:?}'. Free-text input is not accepted here; \
                     use explicit governance commands (/plan accept, /tasks accept, /authorize) to advance."
                    ),
                },
            );
            true
        }
        // Executing, terminal, or blocked stages: legacy continuation.
        _ => false,
    }
}

/// Route a governance decision through the canonical PreExecutionCoordinator.
///
/// The TUI requests; the runtime decides. Coordinator errors are surfaced as
/// explicit interaction errors and persisted as system turns; nothing is
/// silently ignored and nothing bypasses authorization.
async fn handle_lifecycle_action(
    action: ApplicationAction,
    runtime: &mut Arc<AppRuntime>,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &UnboundedSender<InteractionEvent>,
) {
    if matches!(
        action,
        ApplicationAction::ExecutionAuthorizationSubmitted { .. }
    ) && active_execution.is_some()
    {
        emit(
            event_tx,
            InteractionEvent::Error {
                message: "A mission is already currently executing in this session.".to_string(),
            },
        );
        return;
    }
    let effective_action = fill_lifecycle_session_id(action, &session.id.to_string());
    let coordinator = runtime.create_pre_execution_coordinator();
    match coordinator
        .handle_action(effective_action, "operator")
        .await
    {
        Ok(resp) => {
            emit_lifecycle_response(resp, runtime, session, active_execution, event_tx).await;
        }
        Err(err) => {
            let message = format!("Lifecycle error: {err}");
            if let Ok(seq) = session_repo.next_sequence(session.id).await {
                let turn = ConversationTurn::SystemMessage {
                    id: uuid::Uuid::now_v7(),
                    sequence: seq,
                    content: message.clone(),
                    created_at: chrono::Utc::now(),
                };
                let _ = session_repo.append_turn(session.id, &turn).await;
            }
            emit(event_tx, InteractionEvent::Error { message });
        }
    }
}

/// Default the action session id to the active bridge session when absent.
fn fill_lifecycle_session_id(action: ApplicationAction, sid: &str) -> ApplicationAction {
    let or_active = |opt: Option<String>| opt.or_else(|| Some(sid.to_string()));
    match action {
        ApplicationAction::IntentInitRequested {
            session_id,
            raw_prompt,
        } => ApplicationAction::IntentInitRequested {
            session_id: or_active(session_id),
            raw_prompt,
        },
        ApplicationAction::QuestionAnswerSubmitted {
            session_id,
            question_id,
            answer,
        } => ApplicationAction::QuestionAnswerSubmitted {
            session_id: or_active(session_id),
            question_id,
            answer,
        },
        ApplicationAction::PlanEditRequested {
            session_id,
            plan_json,
        } => ApplicationAction::PlanEditRequested {
            session_id: or_active(session_id),
            plan_json,
        },
        ApplicationAction::PlanRevisionRequested {
            session_id,
            feedback,
        } => ApplicationAction::PlanRevisionRequested {
            session_id: or_active(session_id),
            feedback,
        },
        ApplicationAction::PlanRegenerateRequested { session_id } => {
            ApplicationAction::PlanRegenerateRequested {
                session_id: or_active(session_id),
            }
        }
        ApplicationAction::PlanAcceptRequested { session_id } => {
            ApplicationAction::PlanAcceptRequested {
                session_id: or_active(session_id),
            }
        }
        ApplicationAction::PlanRejectRequested { session_id, reason } => {
            ApplicationAction::PlanRejectRequested {
                session_id: or_active(session_id),
                reason,
            }
        }
        ApplicationAction::TaskEditRequested {
            session_id,
            task_json,
        } => ApplicationAction::TaskEditRequested {
            session_id: or_active(session_id),
            task_json,
        },
        ApplicationAction::TaskAddRequested {
            session_id,
            task_json,
        } => ApplicationAction::TaskAddRequested {
            session_id: or_active(session_id),
            task_json,
        },
        ApplicationAction::TaskRemoveRequested {
            session_id,
            task_id,
        } => ApplicationAction::TaskRemoveRequested {
            session_id: or_active(session_id),
            task_id,
        },
        ApplicationAction::TaskRegenerateRequested {
            session_id,
            feedback,
        } => ApplicationAction::TaskRegenerateRequested {
            session_id: or_active(session_id),
            feedback,
        },
        ApplicationAction::TasksAcceptRequested { session_id } => {
            ApplicationAction::TasksAcceptRequested {
                session_id: or_active(session_id),
            }
        }
        ApplicationAction::ExecutionAuthorizationSubmitted {
            session_id,
            decision,
            reason,
        } => ApplicationAction::ExecutionAuthorizationSubmitted {
            session_id: or_active(session_id),
            decision,
            reason,
        },
        other => other,
    }
}

/// Render a canonical coordinator response into the TUI interaction stream.
///
/// Execution starts only here, after authorization, exact-artifact
/// materialization, and the durable StartExecution transition — the same
/// boundary the CLI runner enforces. Any failure fails closed with an
/// explicit error; the TUI never synthesizes approval or completion.
async fn emit_lifecycle_response(
    resp: PreExecutionResponse,
    runtime: &mut Arc<AppRuntime>,
    session: &mut Session,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &UnboundedSender<InteractionEvent>,
) {
    match resp {
        PreExecutionResponse::QuestionsRequired {
            session_id,
            questions,
        } => {
            emit(
                event_tx,
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                },
            );
        }
        PreExecutionResponse::PlanForReview {
            session_id,
            revision,
        } => {
            let content_hash = PlanRevision::compute_content_hash(&revision.content);
            emit(
                event_tx,
                InteractionEvent::PlanForReview {
                    session_id,
                    revision: revision.revision,
                    plan_id: revision.plan_id.clone(),
                    objective: revision.content.objective.clone(),
                    task_count: revision.content.tasks.len(),
                    content_hash: Some(content_hash),
                },
            );
        }
        PreExecutionResponse::TasksForReview {
            session_id,
            revision,
        } => {
            let content_hash = TaskRevision::compute_tasks_hash(&revision.tasks);
            emit(
                event_tx,
                InteractionEvent::TasksForReview {
                    session_id,
                    plan_revision: revision.plan_revision,
                    task_revision: revision.revision,
                    task_count: revision.tasks.len(),
                    content_hash: Some(content_hash),
                },
            );
        }
        PreExecutionResponse::AuthorizationRequested {
            session_id,
            plan_revision,
            task_revision,
            message,
        } => {
            emit(
                event_tx,
                InteractionEvent::AuthorizationRequired {
                    session_id,
                    plan_revision,
                    task_revision,
                    message,
                },
            );
        }
        PreExecutionResponse::ReadyToExecute {
            session_id,
            mut plan,
            tasks,
            authorization,
        } => {
            // Surface authorization first: authorized does not mean executing.
            emit(
                event_tx,
                InteractionEvent::ExecutionReady {
                    session_id: session_id.clone(),
                    authorization_id: authorization.id.to_string(),
                    plan_revision: authorization.plan_revision,
                    task_revision: authorization.task_revision,
                },
            );

            plan.tasks = tasks;
            let objective = plan.objective.clone();
            // The mission identity must be the session's REAL active mission:
            // fabricating one would bind the task graph to a mission that
            // does not exist. Fail closed instead.
            let Some(mission_id) = session.active_mission_id else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message:
                            "Execution refused: session has no active mission to materialize for"
                                .to_string(),
                    },
                );
                return;
            };

            // Ensure the mission row exists before materialization (FK boundary).
            let mission_repo = SqliteMissionRepository::new(runtime.pool().clone());
            match mission_repo.get(mission_id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let mission = Mission::new(mission_id, objective.clone());
                    if let Err(e) = mission_repo.insert(&mission).await {
                        let err_str = e.to_string().to_lowercase();
                        if !err_str.contains("unique") && !err_str.contains("already exists") {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Mission persistence failed for mission {mission_id}: {e}"
                                    ),
                                },
                            );
                            return;
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Mission lookup failed for mission {mission_id}: {e}"),
                        },
                    );
                    return;
                }
            }

            // Exact-artifact materialization: revision and content-hash bound.
            let materializer =
                crate::dag::materializer::TaskGraphMaterializer::new(runtime.pool().clone());
            let graph = match materializer
                .materialize_authorized(
                    mission_id,
                    &plan,
                    authorization.plan_revision,
                    authorization.task_revision,
                    &authorization,
                )
                .await
            {
                Ok(graph) => graph,
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to materialize authorized TaskGraph: {e}"),
                        },
                    );
                    return;
                }
            };

            let summaries = graph.task_summaries();
            emit(
                event_tx,
                InteractionEvent::TasksMaterialized {
                    graph_id: graph.id.to_string(),
                    revision: graph.revision,
                    tasks: summaries.clone(),
                },
            );
            let bus: Arc<dyn EventBus> = runtime.event_bus().clone();
            let _ = crate::scheduler::events::emit_graph_materialized(
                &bus,
                0,
                mission_id,
                graph.id,
                graph.revision,
                graph.tasks.len(),
                summaries,
            )
            .await;

            // INVARIANT D — StartExecution: persist Executing only after the
            // materialization commit, before handing off to the controller.
            // Pure law first, persistence second (D2): durable write goes
            // through the validated transition seam.
            let coordinator = runtime.create_pre_execution_coordinator();
            let lifecycle_repo = coordinator.lifecycle_repo();
            if let Ok(Some(ls)) = lifecycle_repo.load_lifecycle_state(&session_id).await {
                if ls.stage == LifecycleStage::ExecutionAuthorized {
                    if let Err(e) = lifecycle_repo
                        .save_validated_transition(
                            &session_id,
                            LifecycleStage::ExecutionAuthorized,
                            crate::state_machine::lifecycle::LifecycleEvent::StartExecution,
                        )
                        .await
                    {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Failed to persist Executing lifecycle state: {e}"
                                ),
                            },
                        );
                        return;
                    }
                }
            }

            // Non-blocking supervised execution task:
            // Spawn the mission execution so the bridge worker immediately resumes
            // processing kernel events and TUI actions (/cancel, rendering, etc.)!
            let exec_token = CancellationToken::new();
            let exec_token_clone = exec_token.clone();
            let rt = runtime.clone();
            let tx = event_tx.clone();
            let mid = mission_id;
            let obj = objective.clone();

            let handle = tokio::spawn(async move {
                tokio::select! {
                    _ = exec_token_clone.cancelled() => {
                        let _ = rt.cancel_mission(mid, "Execution task cancelled").await;
                    }
                    // The boundary independently re-verifies this artifact
                    // against live state before side effects begin.
                    res = rt.run_authorized_mission(mid, &obj, authorization.clone()) => {
                        match res {
                            Ok(summary) => {
                                let text = format!(
                                    "Mission '{}' status: {} (tasks completed: {})",
                                    summary.mission_id, summary.status, summary.tasks_completed
                                );
                                if summary.status == "Completed" {
                                    emit(&tx, InteractionEvent::CommandOutput { text });
                                    emit(
                                        &tx,
                                        InteractionEvent::Completion {
                                            summary: format!(
                                                "Mission '{}' completed ({} tasks)",
                                                summary.mission_id, summary.tasks_completed
                                            ),
                                        },
                                    );
                                } else {
                                    emit(
                                        &tx,
                                        InteractionEvent::Error {
                                            message: format!("Execution finished without completion: {text}"),
                                        },
                                    );
                                }
                            }
                            Err(e) => {
                                emit(
                                    &tx,
                                    InteractionEvent::Error {
                                        message: format!("Execution failed: {e}"),
                                    },
                                );
                            }
                        }
                    }
                }
            });

            *active_execution = Some(ActiveExecution {
                mission_id,
                join_handle: handle,
                cancel_token: exec_token,
            });
        }
        PreExecutionResponse::Terminated {
            session_id,
            stage,
            reason,
        } => {
            emit(
                event_tx,
                InteractionEvent::LifecycleTerminated {
                    session_id,
                    stage: format!("{stage:?}"),
                    reason,
                },
            );
        }
    }
}
