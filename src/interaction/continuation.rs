//! runtime-layer conversational execution for submitted user text.
//!
//! this module owns what the tui bridge must never own: end-to-end handling
//! of a natural-language submission — mention injection, session turn
//! persistence, governed front-door routing (pre_execution_coordinator),
//! and supervised mission execution after authorization via autonomy_controller.
//!
//! the tui bridge (`crate::tui::runtime_bridge`) only translates: it turns
//! `application_action::user_text_submitted` into a call here, and turns the
//! emitted `interaction_event`s into projection updates. execution truth and
//! governance stay in the runtime/interaction layers.
//!
//! core rule: runtime owns truth. tui owns presentation.

use crate::events::bus::EventBus;
use crate::interaction::action::ApplicationAction;
use crate::interaction::events::InteractionEvent;
use crate::interaction::mentions::{MentionParser, ParsedUserMessage};
use crate::interaction::session::{ConversationTurn, Session, SqliteSessionRepository};
use crate::persistence::sqlite::repositories::SqliteMissionRepository;
use crate::planning::review::{PlanRevision, PreExecutionResponse, TaskRevision};
use crate::runtime::AppRuntime;
use crate::state::Mission;
use crate::state_machine::lifecycle::LifecycleStage;
use crate::tui::channel::TuiInteractionSender;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Supervised background mission execution handle.
///
/// Owned by the bridge worker loop (supervision + cancellation); filled in
/// here when authorized execution spawns. Carries only control handles —
/// never duplicated mission state.
pub struct ActiveExecution {
    pub join_handle: tokio::task::JoinHandle<()>,
    pub cancel_token: CancellationToken,
}

/// Upper bound for one natural-language request through the governed front
/// door plus the legacy agent-engine continuation.
///
/// This is NOT success simulation: on expiry the bridge emits an explicit
/// `Error` event (a valid terminal outcome) instead of leaving the TUI in
/// a silent Thinking state forever. Genuine long model calls complete well
/// inside the bound; only hung calls hit it, and they fail explicitly.
const USER_REQUEST_TIMEOUT_SECS: u64 = 300;

pub(crate) fn emit(event_tx: &TuiInteractionSender, event: InteractionEvent) -> bool {
    match event_tx.try_send(event) {
        Ok(()) => true,
        Err(crate::tui::channel::ActionSendError::Full) => {
            tracing::warn!(
                "TUI bridge: interaction channel saturated; event rejected under backpressure"
            );
            false
        }
        Err(crate::tui::channel::ActionSendError::Closed) => {
            tracing::warn!("TUI bridge: interaction receiver dropped; channel closed");
            false
        }
    }
}

/// Handle one submitted natural-language request end to end.
///
/// Persists the user turn, routes through the governed front door, and —
/// only for executing/terminal continuations — runs the conversational
/// AgentEngine turn, translating every outcome into `InteractionEvent`s.
/// Bounded explicitly: a hung model/provider call surfaces an explicit
/// `Error`, never indefinite silence.
pub async fn handle_user_text_submitted(
    runtime: &mut Arc<AppRuntime>,
    workspace_root: &Path,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    parsed: &ParsedUserMessage,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &TuiInteractionSender,
) {
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
    let mention_ctx = MentionParser::inject_mention_context(workspace_root, &parsed.mentions);
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
            // invariant f — governed front door: all natural-language requests route through
            // the pre_execution_coordinator to autonomy_controller. no unmanaged agent_engine loop.
            if try_governed_front_door(
                runtime,
                workspace_root,
                session,
                parsed,
                active_execution,
                event_tx,
            )
            .await
            {
                return;
            }

            if let Some(active) = active_execution.as_mut() {
                let text = parsed.normalized_prompt().trim().to_string();
                if text.eq_ignore_ascii_case("stop")
                    || text.eq_ignore_ascii_case("cancel")
                    || text.eq_ignore_ascii_case("/cancel")
                {
                    active.cancel_token.cancel();
                    emit(
                        event_tx,
                        InteractionEvent::AssistantOutput {
                            text: "Active mission execution cancelled by operator.".to_string(),
                        },
                    );
                    return;
                }

                // Mid-session steering on the active execution:
                // Apply steering constraint to the active session's intent state.
                let intent_repo = crate::agent::intent_repository::SqliteIntentRepository::new(
                    runtime.pool().clone(),
                );
                if let Ok(Some(mut intent)) = intent_repo.load(session.id).await {
                    intent.apply_steering(crate::agent::intent::SteeringConstraint::new(
                        text.clone(),
                        text.clone(),
                    ));
                    let _ = intent_repo.save(&intent).await;
                }

                emit(
                    event_tx,
                    InteractionEvent::AssistantOutput {
                        text: format!(
                            "Steering constraint recorded for active execution: \"{text}\""
                        ),
                    },
                );
                return;
            }

            emit(
                event_tx,
                InteractionEvent::Error {
                    message: "No active governed execution attached to session. Submit intent through the governed front door.".to_string(),
                },
            );
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

/// governed front door for tui free-text input.
///
/// mirrors the canonical runner rule: new intents enter the pre_execution lifecycle,
/// discovery answers route to the pending question, free text in
/// review/authorization stages is rejected with explicit guidance, and execution
/// continuations route through autonomy_controller.
///
/// returns true when the input was consumed by the governed path.
async fn try_governed_front_door(
    runtime: &mut Arc<AppRuntime>,
    workspace_root: &Path,
    session: &mut Session,
    parsed: &ParsedUserMessage,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &TuiInteractionSender,
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
                // No pending questions: update the existing intent with new clarification text
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
                                message: format!("Error updating intent: {e}"),
                            },
                        );
                    }
                }
                true
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
        // Terminal stages: Completed, Failed, Cancelled, Rejected, Blocked -> initiate a fresh governed intent
        Some(
            LifecycleStage::Completed
            | LifecycleStage::Failed
            | LifecycleStage::Cancelled
            | LifecycleStage::Rejected
            | LifecycleStage::Blocked,
        ) => {
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
                            message: format!("Error starting new governed lifecycle: {e}"),
                        },
                    );
                }
            }
            true
        }
        // Executing stage:
        Some(LifecycleStage::Executing) => {
            if active_execution.is_none() {
                // Not actively executing in this process; guide operator to cancel or re-initialize
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: "Session is recorded as Executing but no active background execution is attached. Use /cancel to reset or start a new session.".to_string(),
                    },
                );
                true
            } else {
                // Active execution exists; handled by steering/cancellation in caller
                false
            }
        }
    }
}

/// Route a governance decision through the canonical PreExecutionCoordinator.
///
/// The TUI requests; the runtime decides. Coordinator errors are surfaced as
/// explicit interaction errors and persisted as system turns; nothing is
/// silently ignored and nothing bypasses authorization.
pub(crate) async fn handle_lifecycle_action(
    action: ApplicationAction,
    runtime: &mut Arc<AppRuntime>,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &TuiInteractionSender,
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
        ApplicationAction::PlanAcceptRequested {
            session_id,
            revision,
            content_hash,
        } => ApplicationAction::PlanAcceptRequested {
            session_id: or_active(session_id),
            revision,
            content_hash,
        },
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
        ApplicationAction::TasksAcceptRequested {
            session_id,
            revision,
            content_hash,
        } => ApplicationAction::TasksAcceptRequested {
            session_id: or_active(session_id),
            revision,
            content_hash,
        },
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
    event_tx: &TuiInteractionSender,
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
            let md = revision.to_markdown();
            emit(
                event_tx,
                InteractionEvent::PlanForReview {
                    session_id,
                    revision: revision.revision,
                    plan_id: revision.plan_id.clone(),
                    objective: revision.content.objective.clone(),
                    task_count: revision.content.tasks.len(),
                    content_hash: Some(content_hash),
                    plan_markdown: Some(md),
                    tasks: revision.content.tasks.clone(),
                },
            );
        }
        PreExecutionResponse::TasksForReview {
            session_id,
            revision,
        } => {
            let content_hash = TaskRevision::compute_tasks_hash(&revision.tasks);
            let md = revision.to_markdown();
            emit(
                event_tx,
                InteractionEvent::TasksForReview {
                    session_id,
                    plan_revision: revision.plan_revision,
                    task_revision: revision.revision,
                    task_count: revision.tasks.len(),
                    content_hash: Some(content_hash),
                    task_markdown: Some(md),
                    tasks: revision.tasks.clone(),
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
