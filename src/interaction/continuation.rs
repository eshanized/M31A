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

use crate::interaction::action::ApplicationAction;
use crate::interaction::events::InteractionEvent;
use crate::interaction::mentions::{MentionParser, ParsedUserMessage};
use crate::interaction::session::{ConversationTurn, Session, SqliteSessionRepository};
use crate::planning::review::{PlanRevision, PreExecutionResponse, TaskRevision};
use crate::runtime::AppRuntime;
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
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let lifecycle_state = coordinator
        .lifecycle_repo()
        .load_lifecycle_state(&sid_str)
        .await
        .unwrap_or(None);
    let governed_stage = lifecycle_state.as_ref().map(|ls| ls.stage);

    match governed_stage {
        // No active lifecycle: start the governed PreExecution lifecycle.
        None => {
            // Planning-choice gate (single contract): explicit planning
            // requests count as consent; a stored session choice is reused;
            // otherwise the operator is asked BEFORE any automatic
            // project-planning workflow may start. The choice is persisted on
            // the session so resume/recovery observes it. Unresolved
            // interactive input defaults to DirectExecution for this turn
            // (never silently opts into project-document generation) while
            // the question stays open.
            let stored = SqliteSessionRepository::read_planning_choice(session);
            let resolved_choice =
                crate::runtime::PlanningChoice::resolve_for_request_with_workspace(
                    &full_prompt,
                    Some(workspace_root),
                    None,
                    stored,
                    false,
                );

            match resolved_choice {
                Some(crate::runtime::PlanningChoice::ProjectPlanning) => {
                    if stored != Some(crate::runtime::PlanningChoice::ProjectPlanning) {
                        let _ = session_repo
                            .set_planning_choice(
                                session.id,
                                crate::runtime::PlanningChoice::ProjectPlanning,
                            )
                            .await;
                        session.metadata["planning_choice"] =
                            serde_json::json!(crate::runtime::PlanningChoice::ProjectPlanning);
                    }
                }
                Some(crate::runtime::PlanningChoice::DirectExecution) => {
                    if stored != Some(crate::runtime::PlanningChoice::DirectExecution) {
                        let _ = session_repo
                            .set_planning_choice(
                                session.id,
                                crate::runtime::PlanningChoice::DirectExecution,
                            )
                            .await;
                        session.metadata["planning_choice"] =
                            serde_json::json!(crate::runtime::PlanningChoice::DirectExecution);
                    }
                }
                None => {
                    // Unresolved planning choice: SUSPEND the current request.
                    // Do not start project Genesis or internal intent while the decision is unresolved.
                    let pending_payload = serde_json::json!({
                        "full_prompt": full_prompt,
                        "raw_text": parsed.raw_text,
                        "normalized_prompt": parsed.normalized_prompt(),
                        "mentions": parsed.mentions,
                        "request_id": parsed.request_id,
                    });
                    let mut metadata = session.metadata.clone();
                    if let Some(obj) = metadata.as_object_mut() {
                        obj.insert("planning_choice_pending".to_string(), pending_payload);
                    }
                    session.metadata = metadata.clone();
                    let metadata_str = serde_json::to_string(&metadata).unwrap_or_default();
                    let now_str = chrono::Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                        "UPDATE sessions SET metadata = ?, updated_at = ? WHERE id = ?",
                    )
                    .bind(&metadata_str)
                    .bind(&now_str)
                    .bind(session.id.as_bytes().as_slice())
                    .execute(runtime.pool())
                    .await;

                    let question = crate::runtime::PlanningChoice::prompt_question().to_string();
                    let decision =
                        crate::interaction::decision::InteractiveDecision::planning_choice(
                            sid_str.clone(),
                            parsed.normalized_prompt(),
                            question.clone(),
                        );
                    match session_repo
                        .set_pending_decision(session.id, &decision)
                        .await
                    {
                        Ok(()) => {
                            session.metadata["pending_decision"] =
                                serde_json::to_value(&decision).unwrap_or_default();
                            emit(
                                event_tx,
                                InteractionEvent::PlanningChoiceRequired {
                                    session_id: sid_str.clone(),
                                    prompt: parsed.normalized_prompt().chars().take(280).collect(),
                                    question,
                                },
                            );
                            emit(event_tx, InteractionEvent::DecisionRequested { decision });
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to persist pending planning choice: {e}"
                                    ),
                                },
                            );
                        }
                    }
                    return true;
                }
            }

            // Request routing by objective classification (Phase 3 Design):
            // Structured DAG planning is reserved for explicit ProjectPlanning,
            // complex multi-step workflows, or mixed sequences (e.g. audit then fix).
            // Direct informational answers, repository questions, read-only research,
            // verification runs, and targeted single-task implementations execute
            // directly under typed role authority with tool visibility bounded to the
            // role envelope.
            let resolved_intent = crate::planning::service::resolve_request_intent(&full_prompt);
            let classification = resolved_intent.classification;
            if resolved_choice == Some(crate::runtime::PlanningChoice::ProjectPlanning)
                || classification == crate::planning::service::ObjectiveClassification::Mixed
                || (classification
                    == crate::planning::service::ObjectiveClassification::Implementation
                    && resolved_choice != Some(crate::runtime::PlanningChoice::DirectExecution))
            {
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
            } else {
                execute_direct_agent_turn(runtime, session, &full_prompt, classification, event_tx)
                    .await
            }
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
        Some(stage @ LifecycleStage::PlanReview) => {
            let trimmed = full_prompt.trim().to_ascii_lowercase();
            if trimmed == "1" || trimmed == "accept" || trimmed == "accept plan" {
                let (rev, hash) =
                    if let Ok(Some(dec)) = session_repo.get_pending_decision(session.id).await {
                        if let crate::interaction::decision::DecisionKind::PlanReview {
                            plan_revision,
                            ref content_hash,
                            ..
                        } = dec.kind
                        {
                            (Some(plan_revision), content_hash.clone())
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                let _ = handle_lifecycle_action(
                    ApplicationAction::PlanAcceptRequested {
                        session_id: Some(sid_str),
                        revision: rev,
                        content_hash: hash,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "2" || trimmed.starts_with("revise") {
                let feedback = if trimmed == "2" {
                    "Operator requested revision".to_string()
                } else {
                    full_prompt
                        .trim()
                        .strip_prefix("revise")
                        .unwrap_or(&full_prompt)
                        .trim()
                        .to_string()
                };
                let _ = handle_lifecycle_action(
                    ApplicationAction::PlanRevisionRequested {
                        session_id: Some(sid_str),
                        feedback,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "3" || trimmed == "regen" || trimmed == "regenerate" {
                let _ = handle_lifecycle_action(
                    ApplicationAction::PlanRegenerateRequested {
                        session_id: Some(sid_str),
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "4" || trimmed == "reject" || trimmed.starts_with("reject") {
                let reason = if trimmed == "4" {
                    "Rejected by operator".to_string()
                } else {
                    full_prompt
                        .trim()
                        .strip_prefix("reject")
                        .unwrap_or("Rejected by operator")
                        .trim()
                        .to_string()
                };
                let _ = handle_lifecycle_action(
                    ApplicationAction::PlanRejectRequested {
                        session_id: Some(sid_str),
                        reason,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!(
                            "Candidate plan review is pending (stage '{stage:?}'). Select an option from the interactive prompt (1: Accept, 2: Revise, 3: Regenerate, 4: Reject)."
                        ),
                    },
                );
                true
            }
        }
        Some(stage @ LifecycleStage::TasksReview) => {
            let trimmed = full_prompt.trim().to_ascii_lowercase();
            if trimmed == "1" || trimmed == "accept" || trimmed == "accept tasks" {
                let (rev, hash) =
                    if let Ok(Some(dec)) = session_repo.get_pending_decision(session.id).await {
                        if let crate::interaction::decision::DecisionKind::TaskReview {
                            task_revision,
                            ref content_hash,
                            ..
                        } = dec.kind
                        {
                            (Some(task_revision), content_hash.clone())
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                let _ = handle_lifecycle_action(
                    ApplicationAction::TasksAcceptRequested {
                        session_id: Some(sid_str),
                        revision: rev,
                        content_hash: hash,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "2" || trimmed == "regen" || trimmed.starts_with("regen") {
                let feedback = if trimmed == "2" || trimmed == "regen" {
                    None
                } else {
                    Some(
                        full_prompt
                            .trim()
                            .strip_prefix("regen")
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                    )
                };
                let _ = handle_lifecycle_action(
                    ApplicationAction::TaskRegenerateRequested {
                        session_id: Some(sid_str),
                        feedback,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "3" || trimmed == "reject" {
                emit(
                    event_tx,
                    InteractionEvent::LifecycleTerminated {
                        session_id: sid_str,
                        stage: "TasksReview".to_string(),
                        reason: "Tasks rejected by operator".to_string(),
                    },
                );
                true
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!(
                            "Candidate task review is pending (stage '{stage:?}'). Select an option from the interactive prompt (1: Accept, 2: Regenerate, 3: Reject)."
                        ),
                    },
                );
                true
            }
        }
        Some(stage @ LifecycleStage::ExecutionAwaitingAuthorization) => {
            let trimmed = full_prompt.trim().to_ascii_lowercase();
            if trimmed == "1" || trimmed == "yes" || trimmed == "authorize" {
                let _ = handle_lifecycle_action(
                    ApplicationAction::ExecutionAuthorizationSubmitted {
                        session_id: Some(sid_str),
                        decision: true,
                        reason: None,
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else if trimmed == "2" || trimmed == "no" || trimmed == "reject" {
                let _ = handle_lifecycle_action(
                    ApplicationAction::ExecutionAuthorizationSubmitted {
                        session_id: Some(sid_str),
                        decision: false,
                        reason: Some("Rejected by operator".to_string()),
                    },
                    runtime,
                    &session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await;
                true
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!(
                            "Execution authorization is pending (stage '{stage:?}'). Select an option from the interactive prompt (1: Authorize, 2: Reject)."
                        ),
                    },
                );
                true
            }
        }
        Some(
            stage @ (LifecycleStage::PlanDraft
            | LifecycleStage::PlanRevision
            | LifecycleStage::PlanAccepted
            | LifecycleStage::TasksDraft
            | LifecycleStage::TasksRevision
            | LifecycleStage::TasksAccepted
            | LifecycleStage::ExecutionAuthorized),
        ) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!(
                        "Governed lifecycle is currently in progress (stage '{stage:?}'). Please wait for the current operation to complete or use the interactive prompt."
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
            // Same planning-choice contract on resumed sessions: an explicit
            // planning request or stored choice applies; otherwise ask before
            // any automatic project planning, defaulting this turn to direct
            // execution (never silent project-doc generation).
            let stored = SqliteSessionRepository::read_planning_choice(session);
            let resolved_choice =
                crate::runtime::PlanningChoice::resolve_for_request_with_workspace(
                    &full_prompt,
                    Some(workspace_root),
                    None,
                    stored,
                    false,
                );

            match resolved_choice {
                Some(crate::runtime::PlanningChoice::ProjectPlanning) => {
                    if stored != Some(crate::runtime::PlanningChoice::ProjectPlanning) {
                        let _ = session_repo
                            .set_planning_choice(
                                session.id,
                                crate::runtime::PlanningChoice::ProjectPlanning,
                            )
                            .await;
                        session.metadata["planning_choice"] =
                            serde_json::json!(crate::runtime::PlanningChoice::ProjectPlanning);
                    }
                }
                Some(crate::runtime::PlanningChoice::DirectExecution) => {
                    if stored != Some(crate::runtime::PlanningChoice::DirectExecution) {
                        let _ = session_repo
                            .set_planning_choice(
                                session.id,
                                crate::runtime::PlanningChoice::DirectExecution,
                            )
                            .await;
                        session.metadata["planning_choice"] =
                            serde_json::json!(crate::runtime::PlanningChoice::DirectExecution);
                    }
                }
                None => {
                    let question = crate::runtime::PlanningChoice::prompt_question().to_string();
                    let decision =
                        crate::interaction::decision::InteractiveDecision::planning_choice(
                            sid_str.clone(),
                            parsed.normalized_prompt(),
                            question.clone(),
                        );
                    match session_repo
                        .set_pending_decision(session.id, &decision)
                        .await
                    {
                        Ok(()) => {
                            session.metadata["pending_decision"] =
                                serde_json::to_value(&decision).unwrap_or_default();
                            emit(
                                event_tx,
                                InteractionEvent::PlanningChoiceRequired {
                                    session_id: sid_str.clone(),
                                    prompt: parsed.normalized_prompt().chars().take(280).collect(),
                                    question,
                                },
                            );
                            emit(event_tx, InteractionEvent::DecisionRequested { decision });
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to persist pending planning choice: {e}"
                                    ),
                                },
                            );
                        }
                    }
                    return true;
                }
            }
            let resolved_intent = crate::planning::service::resolve_request_intent(&full_prompt);
            let classification = resolved_intent.classification;
            if resolved_choice == Some(crate::runtime::PlanningChoice::ProjectPlanning)
                || classification == crate::planning::service::ObjectiveClassification::Mixed
                || (classification
                    == crate::planning::service::ObjectiveClassification::Implementation
                    && resolved_choice != Some(crate::runtime::PlanningChoice::DirectExecution))
            {
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
            } else {
                execute_direct_agent_turn(runtime, session, &full_prompt, classification, event_tx)
                    .await
            }
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

/// Execute a direct turn through `AgentEngine` under typed role authority and
/// bounded tool visibility, without generating an unnecessary structured DAG.
async fn execute_direct_agent_turn(
    runtime: &Arc<AppRuntime>,
    session: &mut Session,
    full_prompt: &str,
    classification: crate::planning::service::ObjectiveClassification,
    event_tx: &TuiInteractionSender,
) -> bool {
    let mut engine = runtime.create_agent_engine(session.id);
    match classification {
        crate::planning::service::ObjectiveClassification::DirectAnswer => {
            // General knowledge question: no repository inspection or executable action required.
            // Expose ZERO tools to model for tool-free natural language answer.
            let all_tools = engine.tool_authority_scope().visible_tools();
            engine = engine.with_denied_tools(all_tools);
        }
        crate::planning::service::ObjectiveClassification::RepositoryInspection
        | crate::planning::service::ObjectiveClassification::RepositoryResearch => {
            // Read-only inspection and research: expose only read-only inspection tools.
            engine =
                engine.with_role_authority(crate::state_machine::agent::AgentRole::researcher());
        }
        crate::planning::service::ObjectiveClassification::RepositoryAudit => {
            // Audit: read-only audit tools.
            engine = engine.with_role_authority(crate::state_machine::agent::AgentRole::auditor());
        }
        crate::planning::service::ObjectiveClassification::Verification => {
            // Verification: verification tools only (cargo.test, test_runner, evidence).
            engine = engine.with_role_authority(crate::state_machine::agent::AgentRole::verifier());
        }
        crate::planning::service::ObjectiveClassification::Investigation => {
            // Diagnostic / failure investigation: read-only inspection and diagnostics.
            engine =
                engine.with_role_authority(crate::state_machine::agent::AgentRole::diagnostician());
        }
        crate::planning::service::ObjectiveClassification::Implementation
        | crate::planning::service::ObjectiveClassification::Documentation => {
            // Direct implementation / documentation: implementer role with policy gate and approvals.
            engine =
                engine.with_role_authority(crate::state_machine::agent::AgentRole::implementer());
        }
        crate::planning::service::ObjectiveClassification::Mixed => {
            // Unreachable: handled by coordinator.init_intent
        }
    }

    engine.reset_intent_from_prompt(full_prompt);

    let tx = event_tx.clone();
    let run_res = engine
        .run_continuous(|outcome| {
            match outcome {
                crate::agent::AgentTurnOutcome::AssistantText { content } => {
                    emit(
                        &tx,
                        InteractionEvent::AssistantOutput {
                            text: content.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::AssistantCommentary { content } => {
                    emit(
                        &tx,
                        InteractionEvent::ModelActivity {
                            text: content.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::ToolResults { results } => {
                    for r in results {
                        emit(
                            &tx,
                            InteractionEvent::ToolCompleted {
                                call_id: r.call_id.clone(),
                                tool_name: r.tool_name.clone(),
                                success: r.success,
                                output_preview: if r.output.len() > 200 {
                                    format!("{}…", &r.output[..200])
                                } else {
                                    r.output.clone()
                                },
                            },
                        );
                    }
                }
                crate::agent::AgentTurnOutcome::Completed { summary } => {
                    emit(
                        &tx,
                        InteractionEvent::Completion {
                            summary: summary.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::Failed { error } => {
                    emit(
                        &tx,
                        InteractionEvent::Error {
                            message: error.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::WaitingForApproval {
                    request_id,
                    tool_name,
                    parameters,
                } => {
                    emit(
                        &tx,
                        InteractionEvent::ApprovalRequested {
                            request_id: request_id.clone(),
                            tool_name: tool_name.clone(),
                            details: format!("Approval required for tool {tool_name}"),
                            risk_tier: None,
                            parameters_summary: Some(parameters.to_string()),
                            agent_role: None,
                            timeout_seconds: None,
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::WaitingForUser { question, .. } => {
                    emit(
                        &tx,
                        InteractionEvent::AssistantOutput {
                            text: question.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::Cancelled { reason } => {
                    emit(
                        &tx,
                        InteractionEvent::LifecycleTerminated {
                            session_id: session.id.to_string(),
                            stage: "Cancelled".to_string(),
                            reason: reason.clone(),
                        },
                    );
                }
                crate::agent::AgentTurnOutcome::TurnLimitReached { reason }
                | crate::agent::AgentTurnOutcome::BudgetExhausted { reason } => {
                    emit(
                        &tx,
                        InteractionEvent::Error {
                            message: reason.clone(),
                        },
                    );
                }
            }
            Ok(())
        })
        .await;

    if let Some(run_id) = engine.active_mission_id() {
        session.active_run_id = Some(run_id);
    }

    match run_res {
        Ok(_) => true,
        Err(e) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!("Direct execution failed: {e}"),
                },
            );
            true
        }
    }
}

async fn invoke_lifecycle_action(
    action: ApplicationAction,
    runtime: &mut Arc<AppRuntime>,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &TuiInteractionSender,
) -> Result<(), String> {
    Box::pin(handle_lifecycle_action(
        action,
        runtime,
        session_repo,
        session,
        active_execution,
        event_tx,
    ))
    .await
}

/// Handle a submitted interactive decision response.
async fn handle_decision_response(
    resp: &crate::interaction::decision::InteractiveDecisionResponse,
    runtime: &mut Arc<AppRuntime>,
    session_repo: &SqliteSessionRepository,
    session: &mut Session,
    active_execution: &mut Option<ActiveExecution>,
    event_tx: &TuiInteractionSender,
) {
    let sid_str = session.id.to_string();
    if resp.session_id != sid_str {
        emit(
            event_tx,
            InteractionEvent::Error {
                message: format!(
                    "Decision response rejected: session mismatch (expected {sid_str}, received {})",
                    resp.session_id
                ),
            },
        );
        return;
    }

    // Check duplicate/replay resistance
    match session_repo
        .is_decision_completed(session.id, &resp.decision_id)
        .await
    {
        Ok(true) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!(
                        "Duplicate decision submission rejected: decision '{}' was already resolved",
                        resp.decision_id
                    ),
                },
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!("Failed to check completed decision status: {e}"),
                },
            );
            return;
        }
    }

    // Load authoritative pending decision
    let pending_opt = match session_repo.get_pending_decision(session.id).await {
        Ok(opt) => opt,
        Err(e) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!("Failed to load pending decision from persistence: {e}"),
                },
            );
            return;
        }
    };

    let pending = match pending_opt {
        Some(dec) if dec.id == resp.decision_id => dec,
        Some(dec) => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!(
                        "Decision ID mismatch: expected active decision '{}', received '{}'",
                        dec.id, resp.decision_id
                    ),
                },
            );
            return;
        }
        None => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!(
                        "Decision response rejected: no pending decision found for ID '{}'",
                        resp.decision_id
                    ),
                },
            );
            return;
        }
    };

    // Validate response against authoritative decision
    if let Err(val_err) = pending.validate_response(resp) {
        emit(
            event_tx,
            InteractionEvent::Error {
                message: format!("Decision response validation failed: {val_err}"),
            },
        );
        return;
    }

    match &pending.kind {
        crate::interaction::decision::DecisionKind::PlanningChoice => {
            if resp.cancelled || resp.selected_option_id.as_deref() == Some("cancel") {
                match session_repo
                    .finalize_decision(session.id, &resp.decision_id, true)
                    .await
                {
                    Ok(updated_session) => {
                        session.metadata = updated_session.metadata;
                        emit(
                            event_tx,
                            InteractionEvent::DecisionResolved {
                                decision_id: resp.decision_id.clone(),
                                session_id: sid_str.clone(),
                                outcome: "Cancelled by operator".to_string(),
                            },
                        );
                        emit(
                            event_tx,
                            InteractionEvent::AssistantOutput {
                                text: "Planning request cancelled by operator.".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Failed to record planning choice cancellation: {e}"
                                ),
                            },
                        );
                    }
                }
                return;
            }

            let generate_project_docs = match resp.selected_option_id.as_deref() {
                Some("plan") => true,
                Some("direct") => false,
                other => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!(
                                "Invalid option {:?} for planning choice. Expected 'plan', 'direct', or 'cancel'.",
                                other
                            ),
                        },
                    );
                    return;
                }
            };

            let outcome = if generate_project_docs {
                "Project planning selected".to_string()
            } else {
                "Direct execution selected".to_string()
            };

            match invoke_lifecycle_action(
                ApplicationAction::PlanningChoiceSubmitted {
                    session_id: Some(sid_str.clone()),
                    generate_project_docs,
                },
                runtime,
                session_repo,
                session,
                active_execution,
                event_tx,
            )
            .await
            {
                Ok(()) => {
                    match session_repo
                        .finalize_decision(session.id, &resp.decision_id, false)
                        .await
                    {
                        Ok(updated_session) => {
                            session.metadata = updated_session.metadata;
                            emit(
                                event_tx,
                                InteractionEvent::DecisionResolved {
                                    decision_id: resp.decision_id.clone(),
                                    session_id: sid_str.clone(),
                                    outcome,
                                },
                            );
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to record planning choice completion: {e}"
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply planning choice: {e}"),
                        },
                    );
                }
            }
        }

        crate::interaction::decision::DecisionKind::PlanReview {
            plan_revision,
            content_hash,
        } => {
            if resp.cancelled || resp.selected_option_id.as_deref() == Some("reject") {
                let reason = resp
                    .custom_answer
                    .clone()
                    .unwrap_or_else(|| "Plan rejected by operator".to_string());
                let action = ApplicationAction::PlanRejectRequested {
                    session_id: Some(sid_str.clone()),
                    reason: reason.clone(),
                };
                let outcome = format!("Plan rejected: {reason}");
                match invoke_lifecycle_action(
                    action,
                    runtime,
                    session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await
                {
                    Ok(()) => {
                        match session_repo
                            .finalize_decision(session.id, &resp.decision_id, false)
                            .await
                        {
                            Ok(updated_session) => {
                                session.metadata = updated_session.metadata;
                                emit(
                                    event_tx,
                                    InteractionEvent::DecisionResolved {
                                        decision_id: resp.decision_id.clone(),
                                        session_id: sid_str.clone(),
                                        outcome,
                                    },
                                );
                            }
                            Err(e) => {
                                emit(
                                    event_tx,
                                    InteractionEvent::Error {
                                        message: format!(
                                            "Failed to record plan rejection completion: {e}"
                                        ),
                                    },
                                );
                            }
                        }
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to reject plan: {e}"),
                            },
                        );
                    }
                }
                return;
            }

            let (action, outcome) = match resp.selected_option_id.as_deref() {
                Some("accept") => (
                    ApplicationAction::PlanAcceptRequested {
                        session_id: Some(sid_str.clone()),
                        revision: Some(*plan_revision),
                        content_hash: content_hash.clone(),
                    },
                    "Plan accepted".to_string(),
                ),
                Some("revise") => {
                    let feedback = resp.custom_answer.clone().unwrap_or_else(|| {
                        "Operator requested revisions to candidate plan".to_string()
                    });
                    (
                        ApplicationAction::PlanRevisionRequested {
                            session_id: Some(sid_str.clone()),
                            feedback: feedback.clone(),
                        },
                        format!("Plan revision requested: {feedback}"),
                    )
                }
                Some("regen") => (
                    ApplicationAction::PlanRegenerateRequested {
                        session_id: Some(sid_str.clone()),
                    },
                    "Plan regeneration requested".to_string(),
                ),
                other => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!(
                                "Invalid option {:?} for plan review. Expected 'accept', 'revise', 'regen', or 'reject'.",
                                other
                            ),
                        },
                    );
                    return;
                }
            };

            match invoke_lifecycle_action(
                action,
                runtime,
                session_repo,
                session,
                active_execution,
                event_tx,
            )
            .await
            {
                Ok(()) => {
                    match session_repo
                        .finalize_decision(session.id, &resp.decision_id, false)
                        .await
                    {
                        Ok(updated_session) => {
                            session.metadata = updated_session.metadata;
                            emit(
                                event_tx,
                                InteractionEvent::DecisionResolved {
                                    decision_id: resp.decision_id.clone(),
                                    session_id: sid_str.clone(),
                                    outcome,
                                },
                            );
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to record plan review completion: {e}"
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply plan review choice: {e}"),
                        },
                    );
                }
            }
        }

        crate::interaction::decision::DecisionKind::TaskReview {
            plan_revision: _,
            task_revision,
            content_hash,
        } => {
            if resp.cancelled || resp.selected_option_id.as_deref() == Some("reject") {
                match session_repo
                    .finalize_decision(session.id, &resp.decision_id, false)
                    .await
                {
                    Ok(updated_session) => {
                        session.metadata = updated_session.metadata;
                        emit(
                            event_tx,
                            InteractionEvent::DecisionResolved {
                                decision_id: resp.decision_id.clone(),
                                session_id: sid_str.clone(),
                                outcome: "Tasks rejected".to_string(),
                            },
                        );
                        emit(
                            event_tx,
                            InteractionEvent::LifecycleTerminated {
                                session_id: sid_str,
                                stage: "TasksReview".to_string(),
                                reason: "Tasks rejected by operator".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Failed to record tasks rejection completion: {e}"
                                ),
                            },
                        );
                    }
                }
                return;
            }

            let (action, outcome) = match resp.selected_option_id.as_deref() {
                Some("accept") => (
                    ApplicationAction::TasksAcceptRequested {
                        session_id: Some(sid_str.clone()),
                        revision: Some(*task_revision),
                        content_hash: content_hash.clone(),
                    },
                    "Tasks accepted".to_string(),
                ),
                Some("regen") => (
                    ApplicationAction::TaskRegenerateRequested {
                        session_id: Some(sid_str.clone()),
                        feedback: resp.custom_answer.clone(),
                    },
                    "Tasks regeneration requested".to_string(),
                ),
                other => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!(
                                "Invalid option {:?} for tasks review. Expected 'accept', 'regen', or 'reject'.",
                                other
                            ),
                        },
                    );
                    return;
                }
            };

            match invoke_lifecycle_action(
                action,
                runtime,
                session_repo,
                session,
                active_execution,
                event_tx,
            )
            .await
            {
                Ok(()) => {
                    match session_repo
                        .finalize_decision(session.id, &resp.decision_id, false)
                        .await
                    {
                        Ok(updated_session) => {
                            session.metadata = updated_session.metadata;
                            emit(
                                event_tx,
                                InteractionEvent::DecisionResolved {
                                    decision_id: resp.decision_id.clone(),
                                    session_id: sid_str.clone(),
                                    outcome,
                                },
                            );
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to record tasks review completion: {e}"
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply tasks review choice: {e}"),
                        },
                    );
                }
            }
        }

        crate::interaction::decision::DecisionKind::ExecutionAuthorization { .. } => {
            let is_auth =
                !resp.cancelled && resp.selected_option_id.as_deref() == Some("authorize");
            let action = ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(sid_str.clone()),
                decision: is_auth,
                reason: resp.custom_answer.clone(),
            };
            let outcome = if is_auth {
                "Execution authorized".to_string()
            } else {
                "Authorization rejected".to_string()
            };
            match invoke_lifecycle_action(
                action,
                runtime,
                session_repo,
                session,
                active_execution,
                event_tx,
            )
            .await
            {
                Ok(()) => {
                    match session_repo
                        .finalize_decision(session.id, &resp.decision_id, false)
                        .await
                    {
                        Ok(updated_session) => {
                            session.metadata = updated_session.metadata;
                            emit(
                                event_tx,
                                InteractionEvent::DecisionResolved {
                                    decision_id: resp.decision_id.clone(),
                                    session_id: sid_str.clone(),
                                    outcome,
                                },
                            );
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to record authorization completion: {e}"
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply authorization decision: {e}"),
                        },
                    );
                }
            }
        }

        crate::interaction::decision::DecisionKind::Clarification {
            question_id,
            allow_freeform,
        } => {
            if resp.cancelled {
                let has_waiter = crate::interaction::decision::DecisionWaiterRegistry::global()
                    .has_waiter(&resp.decision_id)
                    .await
                    || crate::interaction::decision::DecisionWaiterRegistry::global()
                        .has_waiter(question_id)
                        .await;

                if has_waiter {
                    crate::interaction::decision::DecisionWaiterRegistry::global()
                        .resume(
                            &resp.decision_id,
                            crate::interaction::decision::DecisionResumeOutcome::Cancelled(
                                "Cancelled by operator".to_string(),
                            ),
                        )
                        .await;
                    crate::interaction::decision::DecisionWaiterRegistry::global()
                        .resume(
                            question_id,
                            crate::interaction::decision::DecisionResumeOutcome::Cancelled(
                                "Cancelled by operator".to_string(),
                            ),
                        )
                        .await;
                }

                match session_repo
                    .finalize_decision(session.id, &resp.decision_id, false)
                    .await
                {
                    Ok(updated_session) => {
                        session.metadata = updated_session.metadata;
                        emit(
                            event_tx,
                            InteractionEvent::DecisionResolved {
                                decision_id: resp.decision_id.clone(),
                                session_id: sid_str.clone(),
                                outcome: "Cancelled by operator".to_string(),
                            },
                        );
                        emit(
                            event_tx,
                            InteractionEvent::AssistantOutput {
                                text: "Clarification question cancelled by operator.".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to record question cancellation: {e}"),
                            },
                        );
                    }
                }
                return;
            }

            // Resolve canonical answer
            let answer = if let Some(ref custom) = resp.custom_answer {
                let trimmed = custom.trim().to_string();
                if !*allow_freeform {
                    // When freeform is not permitted, custom answer must match a canonical option
                    let matches = pending.options.iter().any(|o| {
                        let opt_val = o.value.as_deref().unwrap_or(&o.label);
                        opt_val.trim().eq_ignore_ascii_case(&trimmed)
                            || o.label.trim().eq_ignore_ascii_case(&trimmed)
                    });
                    if !matches {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Custom answer '{}' does not match permitted options for question '{}'",
                                    trimmed, question_id
                                ),
                            },
                        );
                        return;
                    }
                }
                trimmed
            } else if let Some(ref opt_id) = resp.selected_option_id {
                pending
                    .resolve_option_value(opt_id)
                    .unwrap_or(opt_id)
                    .to_string()
            } else {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message:
                            "Clarification requires either an option selection or a custom answer"
                                .to_string(),
                    },
                );
                return;
            };

            let has_waiter = crate::interaction::decision::DecisionWaiterRegistry::global()
                .has_waiter(&resp.decision_id)
                .await
                || crate::interaction::decision::DecisionWaiterRegistry::global()
                    .has_waiter(question_id)
                    .await;

            if has_waiter {
                crate::interaction::decision::DecisionWaiterRegistry::global()
                    .resume(
                        &resp.decision_id,
                        crate::interaction::decision::DecisionResumeOutcome::Answer(answer.clone()),
                    )
                    .await;
                crate::interaction::decision::DecisionWaiterRegistry::global()
                    .resume(
                        question_id,
                        crate::interaction::decision::DecisionResumeOutcome::Answer(answer.clone()),
                    )
                    .await;

                match session_repo
                    .finalize_decision(session.id, &resp.decision_id, false)
                    .await
                {
                    Ok(updated_session) => {
                        session.metadata = updated_session.metadata;
                        emit(
                            event_tx,
                            InteractionEvent::DecisionResolved {
                                decision_id: resp.decision_id.clone(),
                                session_id: sid_str.clone(),
                                outcome: format!("Answered: {answer}"),
                            },
                        );
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Failed to record question answer completion: {e}"
                                ),
                            },
                        );
                    }
                }
            } else {
                let action = ApplicationAction::QuestionAnswerSubmitted {
                    session_id: Some(sid_str.clone()),
                    question_id: question_id.clone(),
                    answer: answer.clone(),
                };
                let outcome = format!("Answered: {answer}");
                match invoke_lifecycle_action(
                    action,
                    runtime,
                    session_repo,
                    session,
                    active_execution,
                    event_tx,
                )
                .await
                {
                    Ok(()) => {
                        match session_repo
                            .finalize_decision(session.id, &resp.decision_id, false)
                            .await
                        {
                            Ok(updated_session) => {
                                session.metadata = updated_session.metadata;
                                emit(
                                    event_tx,
                                    InteractionEvent::DecisionResolved {
                                        decision_id: resp.decision_id.clone(),
                                        session_id: sid_str.clone(),
                                        outcome,
                                    },
                                );
                            }
                            Err(e) => {
                                emit(
                                    event_tx,
                                    InteractionEvent::Error {
                                        message: format!(
                                            "Failed to record question answer completion: {e}"
                                        ),
                                    },
                                );
                            }
                        }
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!("Failed to submit question answer: {e}"),
                            },
                        );
                    }
                }
            }
        }

        crate::interaction::decision::DecisionKind::PermissionApproval { .. } => {
            let decision = if resp.cancelled || resp.selected_option_id.as_deref() == Some("reject")
            {
                crate::tui::approval::ApprovalDecision::Reject
            } else if resp.selected_option_id.as_deref() == Some("approve_always") {
                crate::tui::approval::ApprovalDecision::ApproveAlways
            } else {
                crate::tui::approval::ApprovalDecision::ApproveOnce
            };

            let action = ApplicationAction::ApprovalDecision {
                request_id: resp.decision_id.clone(),
                decision,
            };
            let outcome = format!("{decision:?}");
            match invoke_lifecycle_action(
                action,
                runtime,
                session_repo,
                session,
                active_execution,
                event_tx,
            )
            .await
            {
                Ok(()) => {
                    match session_repo
                        .finalize_decision(session.id, &resp.decision_id, false)
                        .await
                    {
                        Ok(updated_session) => {
                            session.metadata = updated_session.metadata;
                            emit(
                                event_tx,
                                InteractionEvent::DecisionResolved {
                                    decision_id: resp.decision_id.clone(),
                                    session_id: sid_str.clone(),
                                    outcome,
                                },
                            );
                        }
                        Err(e) => {
                            emit(
                                event_tx,
                                InteractionEvent::Error {
                                    message: format!(
                                        "Failed to record approval decision completion: {e}"
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to apply approval decision: {e}"),
                        },
                    );
                }
            }
        }

        other => {
            emit(
                event_tx,
                InteractionEvent::Error {
                    message: format!("Unhandled decision kind '{:?}'", other),
                },
            );
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
) -> Result<(), String> {
    if let ApplicationAction::DecisionResponseSubmitted(resp) = &action {
        Box::pin(handle_decision_response(
            resp,
            runtime,
            session_repo,
            session,
            active_execution,
            event_tx,
        ))
        .await;
        return Ok(());
    }

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
        return Err("A mission is already currently executing in this session.".to_string());
    }
    // Planning-choice answers are handled directly here (persisted on the
    // session, recoverable across resume); they never enter the
    // plan/task/authorization coordinator stages.
    if let ApplicationAction::PlanningChoiceSubmitted {
        session_id: _,
        generate_project_docs,
    } = &action
    {
        let choice = if *generate_project_docs {
            crate::runtime::PlanningChoice::ProjectPlanning
        } else {
            crate::runtime::PlanningChoice::DirectExecution
        };
        let pending = session.metadata.get("planning_choice_pending").cloned();
        match session_repo.set_planning_choice(session.id, choice).await {
            Ok(()) => {
                session.metadata["planning_choice"] = serde_json::json!(choice);
                if let Some(obj) = session.metadata.as_object_mut() {
                    obj.remove("planning_choice_pending");
                }

                if let Some(pending_req) = pending {
                    let resume_prompt = pending_req
                        .get("full_prompt")
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            pending_req
                                .get("normalized_prompt")
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or_default()
                        .to_string();

                    let desc = if *generate_project_docs {
                        "Project-planning documents will be synthesized under .m31a/project/."
                    } else {
                        "Working directly on this request without generating project-level planning documents."
                    };
                    emit(
                        event_tx,
                        InteractionEvent::AssistantOutput {
                            text: format!("Planning choice recorded: {desc} Resuming request..."),
                        },
                    );

                    let sid_str = session.id.to_string();
                    let workspace_root = runtime.workspace_root().to_path_buf();
                    if *generate_project_docs {
                        let gen_req = crate::workflow::genesis::GenesisRequest::new(
                            resume_prompt.clone(),
                            &workspace_root,
                        )
                        .with_mode(crate::workflow::genesis::GenesisMode::AutoDetect);
                        match runtime.run_genesis(&gen_req).await {
                            Ok(outcome) => {
                                let text = format!(
                                    "Project Genesis planning complete for '{}'. Roadmap lowered to workflow '{}' with {} phases. Registered artifacts: {}.",
                                    outcome.charter.project_name,
                                    outcome.workflow_definition.id,
                                    outcome.planning.roadmap.phases.len(),
                                    outcome.registered_artifacts.len(),
                                );
                                emit(event_tx, InteractionEvent::CommandOutput { text });

                                // BUG-05: Resume original intent after project document generation
                                let coordinator = runtime.create_pre_execution_coordinator();
                                match coordinator
                                    .init_intent(&sid_str, &resume_prompt, "operator")
                                    .await
                                {
                                    Ok(resp) => {
                                        emit_lifecycle_response(
                                            resp,
                                            runtime,
                                            session,
                                            active_execution,
                                            event_tx,
                                        )
                                        .await;
                                        Ok(())
                                    }
                                    Err(e) => {
                                        emit(
                                            event_tx,
                                            InteractionEvent::Error {
                                                message: format!(
                                                    "Error starting governed lifecycle: {e}"
                                                ),
                                            },
                                        );
                                        Err(format!("Error starting governed lifecycle: {e}"))
                                    }
                                }
                            }
                            Err(e) => {
                                emit(
                                    event_tx,
                                    InteractionEvent::Error {
                                        message: format!("Genesis error: {e}"),
                                    },
                                );
                                Err(format!("Genesis error: {e}"))
                            }
                        }
                    } else {
                        let coordinator = runtime.create_pre_execution_coordinator();
                        match coordinator
                            .init_intent(&sid_str, &resume_prompt, "operator")
                            .await
                        {
                            Ok(resp) => {
                                emit_lifecycle_response(
                                    resp,
                                    runtime,
                                    session,
                                    active_execution,
                                    event_tx,
                                )
                                .await;
                                Ok(())
                            }
                            Err(e) => {
                                emit(
                                    event_tx,
                                    InteractionEvent::Error {
                                        message: format!("Error starting governed lifecycle: {e}"),
                                    },
                                );
                                Err(format!("Error starting governed lifecycle: {e}"))
                            }
                        }
                    }
                } else {
                    let text = if *generate_project_docs {
                        "Planning choice recorded: project-planning documents will be generated under .m31a/project/ on your next planning request (or run /genesis now)."
                    } else {
                        "Planning choice recorded: working directly without project-level planning documents. Internal task decomposition continues under existing authorization and sandbox rules."
                    };
                    emit(
                        event_tx,
                        InteractionEvent::AssistantOutput {
                            text: text.to_string(),
                        },
                    );
                    Ok(())
                }
            }
            Err(e) => {
                emit(
                    event_tx,
                    InteractionEvent::Error {
                        message: format!("Failed to persist planning choice: {e}"),
                    },
                );
                Err(format!("Failed to persist planning choice: {e}"))
            }
        }
    } else {
        let effective_action = fill_lifecycle_session_id(action, &session.id.to_string());
        let coordinator = runtime.create_pre_execution_coordinator();
        match coordinator
            .handle_action(effective_action, "operator")
            .await
        {
            Ok(resp) => {
                emit_lifecycle_response(resp, runtime, session, active_execution, event_tx).await;
                Ok(())
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
                Err(err.to_string())
            }
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
        ApplicationAction::PlanningChoiceSubmitted {
            session_id,
            generate_project_docs,
        } => ApplicationAction::PlanningChoiceSubmitted {
            session_id: or_active(session_id),
            generate_project_docs,
        },
        ApplicationAction::DecisionResponseSubmitted(mut resp) => {
            if resp.session_id.is_empty() {
                resp.session_id = sid.to_string();
            }
            ApplicationAction::DecisionResponseSubmitted(resp)
        }
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
                    session_id: session_id.clone(),
                    questions: questions.clone(),
                },
            );
            if let Some(first_q) = questions.first() {
                let decision =
                    crate::interaction::decision::InteractiveDecision::discovery_question(
                        session_id, first_q,
                    );
                let session_repo = crate::interaction::session::SqliteSessionRepository::new(
                    runtime.pool().clone(),
                );
                match session_repo
                    .set_pending_decision(session.id, &decision)
                    .await
                {
                    Ok(()) => {
                        session.metadata["pending_decision"] =
                            serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null);
                        emit(event_tx, InteractionEvent::DecisionRequested { decision });
                    }
                    Err(e) => {
                        emit(
                            event_tx,
                            InteractionEvent::Error {
                                message: format!(
                                    "Failed to persist discovery question decision: {e}"
                                ),
                            },
                        );
                    }
                }
            }
        }
        PreExecutionResponse::PlanForReview {
            session_id,
            revision,
        } => {
            let content_hash = PlanRevision::compute_content_hash(&revision.content);
            let md = revision.to_markdown();
            let decision = crate::interaction::decision::InteractiveDecision::plan_review(
                session_id.clone(),
                revision.revision,
                revision.content.objective.clone(),
                revision.content.tasks.len(),
                Some(content_hash.clone()),
            );
            let session_repo =
                crate::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
            match session_repo
                .set_pending_decision(session.id, &decision)
                .await
            {
                Ok(()) => {
                    session.metadata["pending_decision"] =
                        serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null);

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
                    emit(event_tx, InteractionEvent::DecisionRequested { decision });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to persist plan review decision: {e}"),
                        },
                    );
                }
            }
        }
        PreExecutionResponse::TasksForReview {
            session_id,
            revision,
        } => {
            let content_hash = TaskRevision::compute_tasks_hash(&revision.tasks);
            let md = revision.to_markdown();
            let decision = crate::interaction::decision::InteractiveDecision::tasks_review(
                session_id.clone(),
                revision.plan_revision,
                revision.revision,
                revision.tasks.len(),
                Some(content_hash.clone()),
            );
            let session_repo =
                crate::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
            match session_repo
                .set_pending_decision(session.id, &decision)
                .await
            {
                Ok(()) => {
                    session.metadata["pending_decision"] =
                        serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null);

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
                    emit(event_tx, InteractionEvent::DecisionRequested { decision });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to persist tasks review decision: {e}"),
                        },
                    );
                }
            }
        }
        PreExecutionResponse::AuthorizationRequested {
            session_id,
            plan_revision,
            task_revision,
            message,
        } => {
            let decision =
                crate::interaction::decision::InteractiveDecision::execution_authorization(
                    session_id.clone(),
                    plan_revision,
                    task_revision,
                    message.clone(),
                );
            let session_repo =
                crate::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
            match session_repo
                .set_pending_decision(session.id, &decision)
                .await
            {
                Ok(()) => {
                    session.metadata["pending_decision"] =
                        serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null);

                    emit(
                        event_tx,
                        InteractionEvent::AuthorizationRequired {
                            session_id,
                            plan_revision,
                            task_revision,
                            message,
                        },
                    );
                    emit(event_tx, InteractionEvent::DecisionRequested { decision });
                }
                Err(e) => {
                    emit(
                        event_tx,
                        InteractionEvent::Error {
                            message: format!("Failed to persist authorization decision: {e}"),
                        },
                    );
                }
            }
        }
        PreExecutionResponse::ReadyToExecute {
            session_id,
            mut plan,
            tasks,
            authorization,
        } => {
            let session_repo =
                crate::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
            let _ = session_repo.clear_pending_decision(session.id).await;
            if let Some(obj) = session.metadata.as_object_mut() {
                obj.remove("pending_decision");
            }

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

            // Resolve or establish canonical ExecutionRun identity on the session
            let run_id = match session.active_run_id.or(session.active_mission_id) {
                Some(mid) => mid,
                None => {
                    let new_id = crate::ids::ExecutionRunId::new();
                    session.active_run_id = Some(new_id);
                    session.active_mission_id = Some(new_id);
                    let session_repo = crate::interaction::session::SqliteSessionRepository::new(
                        runtime.pool().clone(),
                    );
                    let _ = session_repo.set_active_run(session.id, new_id).await;
                    new_id
                }
            };

            // Non-blocking supervised execution task:
            // Spawn the authorized execution so the bridge worker immediately resumes
            // processing kernel events and TUI actions (/cancel, rendering, etc.)!
            let exec_token = CancellationToken::new();
            let exec_token_clone = exec_token.clone();
            let rt = runtime.clone();
            let tx = event_tx.clone();
            let sid = session.id;
            let plan_clone = plan.clone();
            let auth = authorization.clone();

            let handle = tokio::spawn(async move {
                tokio::select! {
                    _ = exec_token_clone.cancelled() => {
                        let _ = rt.cancel_execution(run_id, "Execution task cancelled").await;
                    }
                    res = rt.start_authorized_execution(sid, &plan_clone, auth) => {
                        match res {
                            Ok(summary) => {
                                let text = format!(
                                    "Execution '{}' status: {} (tasks completed: {})",
                                    summary.mission_id, summary.status, summary.tasks_completed
                                );
                                if summary.status == "Completed" {
                                    emit(&tx, InteractionEvent::CommandOutput { text });
                                    emit(
                                        &tx,
                                        InteractionEvent::Completion {
                                            summary: format!(
                                                "Execution '{}' completed ({} tasks)",
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
            let session_repo =
                crate::interaction::session::SqliteSessionRepository::new(runtime.pool().clone());
            let _ = session_repo.clear_pending_decision(session.id).await;
            if let Some(obj) = session.metadata.as_object_mut() {
                obj.remove("pending_decision");
            }
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
