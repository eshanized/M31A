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
use crate::config::env::SafeEnvironmentStatus;
use crate::error::M31AError;
use crate::events::bus::{EventBus, EventFilter};
use crate::events::types::EventType;
use crate::ids::SessionId;
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

        // Initialize or resume durable session
        let session = if let Some(sid) = resume_session_id {
            if let Some(s) = session_repo.get_session(sid).await? {
                s
            } else {
                session_repo.create_session(&workspace_root).await?
            }
        } else {
            session_repo.create_session(&workspace_root).await?
        };

        let active_sid = session.id;
        let was_resume = resume_session_id.is_some();

        // Emit initial session event
        let _ = event_tx.send(InteractionEvent::SessionStarted {
            session_id: active_sid,
        });
        if was_resume {
            let _ = event_tx.send(InteractionEvent::SessionResumed {
                session_id: active_sid,
            });
        }

        // Session hydration: historical state + new events = current
        // projection. Load persisted lifecycle + conversation BEFORE
        // subscribing to live events so the TUI reconstructs truth after
        // restart/reconnect instead of depending on post-startup events.
        // Hydration never executes: re-execution requires fresh authorization.
        for ev in hydrate_session_state(&runtime, &session_repo, &session).await {
            let _ = event_tx.send(ev);
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
        graph_repo.find_latest_active_graph().await.unwrap_or(None)
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
            let texts: Vec<String> = questions
                .iter()
                .map(|q| {
                    if q.options.is_empty() {
                        q.text.clone()
                    } else {
                        format!("{} Options: {}", q.text, q.options.join(", "))
                    }
                })
                .collect();
            out.push(InteractionEvent::DiscoveryRequired {
                session_id,
                questions: texts,
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
/// The asynchronous background worker driving execution and event routing.
async fn run_bridge_worker(
    mut runtime: Arc<AppRuntime>,
    workspace_root: PathBuf,
    session_repo: SqliteSessionRepository,
    mut session: Session,
    mut action_rx: UnboundedReceiver<ApplicationAction>,
    event_tx: UnboundedSender<InteractionEvent>,
) {
    let command_registry = SlashCommandRegistry::new_standard();
    let mut kernel_rx = runtime.event_bus().subscribe(EventFilter::all()).await;
    let mut cancel_token = CancellationToken::new();

    loop {
        tokio::select! {
            // 1. Process kernel bus events and forward to TUI
            Some(Ok(env)) = kernel_rx.next() => {
                match &env.event_type {
                    EventType::MissionStarted { mission_id, .. } => {
                        let _ = event_tx.send(InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "running".to_string(),
                        });
                    }
                    EventType::MissionCompleted { mission_id } => {
                        let _ = event_tx.send(InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "completed".to_string(),
                        });
                    }
                    EventType::MissionFailed { mission_id, reason } => {
                        let _ = event_tx.send(InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "failed".to_string(),
                        });
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Mission failed: {reason}"),
                        });
                    }
                    EventType::MissionPaused { mission_id, .. } => {
                        let _ = event_tx.send(InteractionEvent::MissionStateChanged {
                            mission_id: *mission_id,
                            status: "paused".to_string(),
                        });
                    }
                    EventType::MissionResumed { mission_id, .. } => {
                        let _ = event_tx.send(InteractionEvent::MissionStateChanged {
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
                        let params = serde_json::to_value(arguments).unwrap_or_default();
                        let _ = event_tx.send(InteractionEvent::ToolStarted {
                            call_id: tool_call_id.to_string(),
                            tool_name: tool_name.clone(),
                            parameters: params,
                        });
                    }
                    EventType::ToolCompleted { tool_call_id, result, .. } => {
                        let _ = event_tx.send(InteractionEvent::ToolCompleted {
                            call_id: tool_call_id.to_string(),
                            tool_name: "".to_string(),
                            success: true,
                            output_preview: result.clone(),
                        });
                    }
                    EventType::ToolFailed { tool_call_id, error, .. } => {
                        let _ = event_tx.send(InteractionEvent::ToolCompleted {
                            call_id: tool_call_id.to_string(),
                            tool_name: "".to_string(),
                            success: false,
                            output_preview: error.clone(),
                        });
                    }
                    EventType::OperatorEscalationRequested { request_id, reason, .. } => {
                        let _ = event_tx.send(InteractionEvent::ApprovalRequested {
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
                        let _ = event_tx.send(InteractionEvent::ApprovalResolved {
                            request_id: request_id.clone(),
                            approved,
                        });
                    }
                    // Verification outcomes flow here as interaction cards; the
                    // direct envelope channel carries the same outcome for the
                    // lifecycle projection (no duplicate cards there).
                    EventType::VerificationCompleted { passed, evidence, .. } => {
                        if *passed {
                            let _ = event_tx.send(InteractionEvent::VerificationPassed {
                                summary: evidence.clone(),
                            });
                        } else {
                            let _ = event_tx.send(InteractionEvent::VerificationFailed {
                                summary: evidence.clone(),
                            });
                        }
                    }
                    EventType::GitStateChanged {
                        workspace_branch,
                        execution_branch,
                        is_clean,
                    } => {
                        let _ = event_tx.send(InteractionEvent::GitStateChanged {
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
                        let _ = event_tx.send(InteractionEvent::ModelUsageUpdated {
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
                        let _ = event_tx.send(InteractionEvent::TasksMaterialized {
                            graph_id: graph_id.to_string(),
                            revision: *revision,
                            tasks: tasks.clone(),
                        });
                    }
                    _ => {}
                }
            }

            // 2. Process actions originating from TUI
            Some(action) = action_rx.recv() => {
                let should_exit = dispatch_bridge_action(
                    action,
                    &mut runtime,
                    &workspace_root,
                    &session_repo,
                    &mut session,
                    &command_registry,
                    &mut cancel_token,
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
    event_tx: &UnboundedSender<InteractionEvent>,
) -> bool {
    match action {
        ApplicationAction::UserTextSubmitted(parsed) => {
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

            let _ = event_tx.send(InteractionEvent::ModelActivity {
                text: "Reasoning on task plan...".to_string(),
            });

            // INVARIANT F — governed front door (same rule as the
            // CLI runner): new intents and discovery answers route through the
            // PreExecutionCoordinator; free text in review/authorization stages
            // is rejected fail-closed; only executing/terminal continuations
            // reach the legacy AgentEngine below.
            if try_governed_front_door(runtime, workspace_root, session, &parsed, event_tx).await {
                return false;
            }

            let (chunk_tx, mut chunk_rx) = unbounded_channel();
            let mut engine = runtime
                .create_agent_engine(session.id)
                .with_stream_sender(chunk_tx);
            let _ = engine.load_session_state().await;

            let _ = event_tx.send(InteractionEvent::ModelActivity {
                text: "Reasoning and executing via AgentEngine...".to_string(),
            });

            let stream_event_tx = event_tx.clone();
            let stream_forwarder = tokio::spawn(async move {
                let mut message_id = uuid::Uuid::now_v7().to_string();
                let mut stream_started = false;
                while let Some(chunk) = chunk_rx.recv().await {
                    match chunk {
                        crate::model::types::StreamChunk::TextDelta(delta) => {
                            if !stream_started {
                                let _ = stream_event_tx.send(InteractionEvent::AssistantStarted {
                                    message_id: message_id.clone(),
                                });
                                stream_started = true;
                            }
                            let _ = stream_event_tx.send(InteractionEvent::AssistantDelta {
                                message_id: message_id.clone(),
                                delta,
                            });
                        }
                        crate::model::types::StreamChunk::ToolCallDelta { name, .. } => {
                            if let Some(tool_name) = name {
                                let _ = stream_event_tx.send(InteractionEvent::ModelActivity {
                                    text: format!("Preparing tool `{tool_name}`..."),
                                });
                            }
                        }
                        crate::model::types::StreamChunk::UsageUpdate(usage) => {
                            let _ = stream_event_tx.send(InteractionEvent::ModelUsageUpdated {
                                invocation_id: None,
                                prompt_tokens: usage.prompt_tokens as u64,
                                completion_tokens: usage.completion_tokens as u64,
                                total_tokens: usage.total_tokens as u64,
                                cost_cents: None,
                            });
                        }
                        crate::model::types::StreamChunk::FinishReason(_) => {
                            if stream_started {
                                let _ = stream_event_tx.send(InteractionEvent::AssistantFinished {
                                    message_id: message_id.clone(),
                                });
                                message_id = uuid::Uuid::now_v7().to_string();
                                stream_started = false;
                            }
                        }
                    }
                }
                if stream_started {
                    let _ =
                        stream_event_tx.send(InteractionEvent::AssistantFinished { message_id });
                }
            });

            let final_state_res = engine
                .run_continuous(|outcome| {
                    match outcome {
                        AgentTurnOutcome::AssistantCommentary { content } => {
                            let _ = event_tx.send(InteractionEvent::AssistantOutput {
                                text: content.clone(),
                            });
                        }
                        AgentTurnOutcome::AssistantText { content } => {
                            let _ = event_tx.send(InteractionEvent::AssistantOutput {
                                text: content.clone(),
                            });
                        }
                        AgentTurnOutcome::ToolResults { results } => {
                            for r in results {
                                let _ = event_tx.send(InteractionEvent::ToolCompleted {
                                    call_id: r.call_id.clone(),
                                    tool_name: r.tool_name.clone(),
                                    success: r.success,
                                    output_preview: r.output.chars().take(200).collect(),
                                });
                            }
                        }
                        AgentTurnOutcome::WaitingForUser { question, .. } => {
                            let _ = event_tx.send(InteractionEvent::AssistantOutput {
                                text: format!("[Question] {question}"),
                            });
                        }
                        AgentTurnOutcome::WaitingForApproval {
                            request_id,
                            tool_name,
                            parameters,
                        } => {
                            let _ = event_tx.send(InteractionEvent::ApprovalRequested {
                                request_id: request_id.clone(),
                                tool_name: tool_name.clone(),
                                details: parameters.to_string(),
                            });
                        }
                        AgentTurnOutcome::Completed { summary } => {
                            // §39: an assistant turn completing is NOT mission
                            // completion. Mission completion originates only
                            // from the verification-gated completion path
                            // (MissionCompleted event). Report the turn
                            // truthfully as assistant output.
                            let _ = event_tx.send(InteractionEvent::AssistantOutput {
                                text: summary.clone(),
                            });
                        }
                        AgentTurnOutcome::Failed { error } => {
                            let _ = event_tx.send(InteractionEvent::VerificationFailed {
                                summary: error.clone(),
                            });
                            let _ = event_tx.send(InteractionEvent::Error {
                                message: error.clone(),
                            });
                        }
                        AgentTurnOutcome::Cancelled { reason } => {
                            let _ = event_tx.send(InteractionEvent::Error {
                                message: format!("Cancelled: {reason}"),
                            });
                        }
                        AgentTurnOutcome::BudgetExhausted { reason } => {
                            let _ = event_tx.send(InteractionEvent::Error {
                                message: format!("Turn budget exhausted: {reason}"),
                            });
                        }
                    }
                    Ok(())
                })
                .await;

            drop(engine);
            let _ = stream_forwarder.await;

            if let Err(e) = final_state_res {
                let _ = event_tx.send(InteractionEvent::Error {
                    message: format!("AgentEngine execution error: {e}"),
                });
            }
        }

        ApplicationAction::SlashCommandSubmitted { command, args } => {
            let cmd_line = format!("/{} {}", command, args.join(" "));
            let env_status = SafeEnvironmentStatus::probe();
            let ctx = CommandContext {
                workspace_root,
                session_id: Some(session.id),
                active_mission_id: session.active_mission_id,
                pool: runtime.pool(),
                event_bus: runtime.event_bus(),
                configured_model: runtime.config().active_model.clone(),
                configured_provider: if env_status.provider_configured {
                    "nvidia_nim".to_string()
                } else {
                    "none".to_string()
                },
                active_profile: runtime
                    .config()
                    .active_profile
                    .clone()
                    .unwrap_or_else(|| "autonomous".to_string()),
            };

            match command_registry.execute_line(&cmd_line, &ctx).await {
                Ok(CommandOutput::Info(txt)) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: txt });
                }
                Ok(CommandOutput::Error(err)) => {
                    let _ = event_tx.send(InteractionEvent::Error { message: err });
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
                        event_tx,
                    ))
                    .await;
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Command failed: {e}"),
                    });
                }
            }
        }

        ApplicationAction::DiffRequested => match runtime.get_git_diff().await {
            Ok(diff) => {
                let _ = event_tx.send(InteractionEvent::CommandOutput { text: diff });
            }
            Err(e) => {
                let _ = event_tx.send(InteractionEvent::Error {
                    message: format!("Diff error: {e}"),
                });
            }
        },

        ApplicationAction::CommitRequested { message } => {
            match runtime
                .commit_changes(session.active_mission_id, message.as_deref())
                .await
            {
                Ok(summary) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: summary });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Commit failed: {e}"),
                    });
                }
            }
        }

        ApplicationAction::StatusRequested => {
            let env_status = SafeEnvironmentStatus::probe();
            let ctx = CommandContext {
                workspace_root,
                session_id: Some(session.id),
                active_mission_id: session.active_mission_id,
                pool: runtime.pool(),
                event_bus: runtime.event_bus(),
                configured_model: runtime.config().active_model.clone(),
                configured_provider: if env_status.provider_configured {
                    "nvidia_nim".to_string()
                } else {
                    "none".to_string()
                },
                active_profile: runtime
                    .config()
                    .active_profile
                    .clone()
                    .unwrap_or_else(|| "autonomous".to_string()),
            };
            if let Ok(CommandOutput::Info(info)) =
                command_registry.execute_line("/status", &ctx).await
            {
                let _ = event_tx.send(InteractionEvent::CommandOutput { text: info });
            }
        }

        ApplicationAction::ClearRequested => {
            let _ = event_tx.send(InteractionEvent::CommandOutput {
                text: "Console cleared.".to_string(),
            });
        }

        ApplicationAction::ClearSessionRequested => {
            let _ = session_repo.clear_conversation(session.id).await;
            let _ = event_tx.send(InteractionEvent::CommandOutput {
                text: "Conversation cleared.".to_string(),
            });
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
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Failed to switch model: {e}"),
                    });
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
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Failed to switch profile: {e}"),
                    });
                }
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
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Failed to apply configuration override: {e}"),
                    });
                }
            }
        }

        ApplicationAction::SessionResumeRequested { session_id } => {
            if let Ok(uuid) = session_id.parse::<uuid::Uuid>() {
                let sid = SessionId::from(uuid);
                match session_repo.get_session(sid).await {
                    Ok(Some(s)) => {
                        *session = s;
                        let _ = event_tx.send(InteractionEvent::CommandOutput {
                            text: format!("Resumed session: {}", session.id),
                        });
                    }
                    Ok(None) => {
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Session '{session_id}' not found."),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Failed to resume session: {e}"),
                        });
                    }
                }
            } else {
                let _ = event_tx.send(InteractionEvent::Error {
                    message: format!("Invalid session ID: '{session_id}'"),
                });
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

            let _ = event_tx.send(InteractionEvent::ApprovalResolved {
                request_id,
                approved,
            });
        }

        ApplicationAction::CancelRequested => {
            cancel_token.cancel();
            *cancel_token = CancellationToken::new();

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

            let _ = event_tx.send(InteractionEvent::CommandOutput {
                text: "Operation cancelled.".to_string(),
            });
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
            handle_lifecycle_action(action, runtime, session_repo, session, event_tx).await;
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
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text });
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
                    let _ = event_tx.send(InteractionEvent::Error { message });
                }
            }
        }

        ApplicationAction::GenesisStateRequested => {
            match runtime.get_genesis_state(".planning").await {
                Ok(Some(state)) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput {
                        text: format!(
                            "Planning state: lifecycle={:?} status={} phase={:?}",
                            state.lifecycle_state, state.approval_status, state.current_phase
                        ),
                    });
                }
                Ok(None) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput {
                        text: "No Project Genesis state found in .planning/STATE.md".to_string(),
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Error inspecting Genesis state: {e}"),
                    });
                }
            }
        }

        ApplicationAction::WorkflowResumeRequested { run_id } => {
            match runtime.handle_workflow_resume(&run_id).await {
                Ok(report) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Workflow resume failed for run {run_id}: {e}"),
                    });
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
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!(
                            "Workflow approval failed for run {run_id}, step {step_key}: {e}"
                        ),
                    });
                }
            }
        }

        ApplicationAction::WorkflowPauseRequested { run_id, reason } => {
            match runtime.pause_workflow_run(&run_id, &reason).await {
                Ok(report) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Workflow pause failed for run {run_id}: {e}"),
                    });
                }
            }
        }

        ApplicationAction::WorkflowCancelRequested { run_id, reason } => {
            match runtime.cancel_workflow_run(&run_id, &reason).await {
                Ok(report) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Workflow cancel failed for run {run_id}: {e}"),
                    });
                }
            }
        }

        ApplicationAction::WorkflowInspectRequested { run_id } => {
            match runtime.inspect_workflow_run(&run_id).await {
                Ok(report) => {
                    let _ = event_tx.send(InteractionEvent::CommandOutput { text: report });
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Workflow inspect failed for run {run_id}: {e}"),
                    });
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
            let _ = event_tx.send(InteractionEvent::CommandOutput { text });
        }
    }

    false
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
                    emit_lifecycle_response(resp, runtime, session, event_tx).await;
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Error starting governed lifecycle: {e}"),
                    });
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
                        emit_lifecycle_response(resp, runtime, session, event_tx).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Error submitting discovery answer: {e}"),
                        });
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
            let _ = event_tx.send(InteractionEvent::Error {
                message: format!(
                    "Governed lifecycle is in stage '{stage:?}'. Free-text input is not accepted here; \
                     use explicit governance commands (/plan accept, /tasks accept, /authorize) to advance."
                ),
            });
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
    event_tx: &UnboundedSender<InteractionEvent>,
) {
    let effective_action = fill_lifecycle_session_id(action, &session.id.to_string());
    let coordinator = runtime.create_pre_execution_coordinator();
    match coordinator
        .handle_action(effective_action, "operator")
        .await
    {
        Ok(resp) => {
            emit_lifecycle_response(resp, runtime, session, event_tx).await;
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
            let _ = event_tx.send(InteractionEvent::Error { message });
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
    event_tx: &UnboundedSender<InteractionEvent>,
) {
    match resp {
        PreExecutionResponse::QuestionsRequired {
            session_id,
            questions,
        } => {
            let texts: Vec<String> = questions
                .iter()
                .map(|q| {
                    if q.options.is_empty() {
                        q.text.clone()
                    } else {
                        format!("{} Options: {}", q.text, q.options.join(", "))
                    }
                })
                .collect();
            let _ = event_tx.send(InteractionEvent::DiscoveryRequired {
                session_id,
                questions: texts,
            });
        }
        PreExecutionResponse::PlanForReview {
            session_id,
            revision,
        } => {
            let content_hash = PlanRevision::compute_content_hash(&revision.content);
            let _ = event_tx.send(InteractionEvent::PlanForReview {
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
            let _ = event_tx.send(InteractionEvent::TasksForReview {
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
            let _ = event_tx.send(InteractionEvent::AuthorizationRequired {
                session_id,
                plan_revision,
                task_revision,
                message,
            });
        }
        PreExecutionResponse::ReadyToExecute {
            session_id,
            mut plan,
            tasks,
            authorization,
        } => {
            // Surface authorization first: authorized does not mean executing.
            let _ = event_tx.send(InteractionEvent::ExecutionReady {
                session_id: session_id.clone(),
                authorization_id: authorization.id.to_string(),
                plan_revision: authorization.plan_revision,
                task_revision: authorization.task_revision,
            });

            plan.tasks = tasks;
            let objective = plan.objective.clone();
            let mission_id = session.active_mission_id.unwrap_or_default();

            // Ensure the mission row exists before materialization (FK boundary).
            let mission_repo = SqliteMissionRepository::new(runtime.pool().clone());
            match mission_repo.get(mission_id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let mission = Mission::new(mission_id, objective.clone());
                    if let Err(e) = mission_repo.insert(&mission).await {
                        let err_str = e.to_string().to_lowercase();
                        if !err_str.contains("unique") && !err_str.contains("already exists") {
                            let _ = event_tx.send(InteractionEvent::Error {
                                message: format!(
                                    "Mission persistence failed for mission {mission_id}: {e}"
                                ),
                            });
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Mission lookup failed for mission {mission_id}: {e}"),
                    });
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
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Failed to materialize authorized TaskGraph: {e}"),
                    });
                    return;
                }
            };

            let summaries = graph.task_summaries();
            let _ = event_tx.send(InteractionEvent::TasksMaterialized {
                graph_id: graph.id.to_string(),
                revision: graph.revision,
                tasks: summaries.clone(),
            });
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
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Failed to persist Executing lifecycle state: {e}"),
                        });
                        return;
                    }
                }
            }

            let _ = event_tx.send(InteractionEvent::CommandOutput {
                text: format!(
                    "Execution started via StartExecution boundary: graph {} with {} tasks.",
                    graph.id,
                    graph.tasks.len()
                ),
            });

            match runtime.run_authorized_mission(mission_id, &objective).await {
                Ok(summary) => {
                    let text = format!(
                        "Mission '{}' status: {} (tasks completed: {})",
                        summary.mission_id, summary.status, summary.tasks_completed
                    );
                    if summary.status == "Completed" {
                        let _ = event_tx.send(InteractionEvent::CommandOutput { text });
                        let _ = event_tx.send(InteractionEvent::Completion {
                            summary: format!(
                                "Mission '{}' completed ({} tasks)",
                                summary.mission_id, summary.tasks_completed
                            ),
                        });
                    } else {
                        let _ = event_tx.send(InteractionEvent::Error {
                            message: format!("Execution finished without completion: {text}"),
                        });
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(InteractionEvent::Error {
                        message: format!("Execution failed: {e}"),
                    });
                }
            }
        }
        PreExecutionResponse::Terminated {
            session_id,
            stage,
            reason,
        } => {
            let _ = event_tx.send(InteractionEvent::LifecycleTerminated {
                session_id,
                stage: format!("{stage:?}"),
                reason,
            });
        }
    }
}
