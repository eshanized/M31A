//! Production action dispatcher wiring WorkerRunner to the 11-stage pipeline (TL-03, D-12).

use async_trait::async_trait;
use std::sync::Arc;

use crate::agent::runner::{ActionDispatcher, ActionRequest, ActionResult};
use crate::kernel::seams::policy::PolicyGate;
use crate::pipeline::runner::ToolPipelineRunner;
use crate::state::intake::AutonomyMode;
use crate::tools::definition::ToolExecutionContext;

/// production action dispatcher executing actions through the non-bypassable 11-stage pipeline (TL-03).
pub struct ProductionActionDispatcher {
    runner: Arc<ToolPipelineRunner>,
    context: ToolExecutionContext,
    policy_gate: Arc<dyn PolicyGate>,
    autonomy_mode: AutonomyMode,
    replan_authority: Option<Arc<crate::recovery::ReplanAuthority>>,
    session_repo: Option<crate::interaction::session::SqliteSessionRepository>,
    session_id: Option<crate::ids::SessionId>,
    event_bus: Option<Arc<dyn crate::events::EventBus>>,
    db_pool: Option<sqlx::SqlitePool>,
}

impl ProductionActionDispatcher {
    /// create a new production action dispatcher.
    pub fn new(
        runner: Arc<ToolPipelineRunner>,
        context: ToolExecutionContext,
        policy_gate: Arc<dyn PolicyGate>,
        autonomy_mode: AutonomyMode,
    ) -> Self {
        Self {
            runner,
            context,
            policy_gate,
            autonomy_mode,
            replan_authority: None,
            session_repo: None,
            session_id: None,
            event_bus: None,
            db_pool: None,
        }
    }

    /// Attach a session repository for interactive decisions.
    pub fn with_session_repo(
        mut self,
        repo: crate::interaction::session::SqliteSessionRepository,
    ) -> Self {
        self.session_repo = Some(repo);
        self
    }

    /// Attach a specific session ID for interactive decisions.
    pub fn with_session_id(mut self, id: crate::ids::SessionId) -> Self {
        self.session_id = Some(id);
        self
    }

    /// Attach an event bus for publishing interaction events.
    pub fn with_event_bus(mut self, bus: Arc<dyn crate::events::EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// attach the canonical replan authority.
    pub fn with_replan_authority(
        mut self,
        replan_authority: Arc<crate::recovery::ReplanAuthority>,
    ) -> Self {
        self.replan_authority = Some(replan_authority);
        self
    }

    /// access the replan authority, if configured.
    pub fn replan_authority(&self) -> Option<&Arc<crate::recovery::ReplanAuthority>> {
        self.replan_authority.as_ref()
    }

    /// Access the pipeline runner.
    pub fn runner(&self) -> &Arc<ToolPipelineRunner> {
        &self.runner
    }

    /// Access the execution context.
    pub fn context(&self) -> &ToolExecutionContext {
        &self.context
    }

    /// Access the policy gate.
    pub fn policy_gate(&self) -> &Arc<dyn PolicyGate> {
        &self.policy_gate
    }

    /// Access the active autonomy mode.
    pub fn autonomy_mode(&self) -> AutonomyMode {
        self.autonomy_mode
    }

    /// Attach a database pool for task graph persistence and policy auditing.
    pub fn with_db_pool(mut self, pool: sqlx::SqlitePool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Access the SQLite database pool if configured.
    pub fn db_pool(&self) -> Option<&sqlx::SqlitePool> {
        self.db_pool.as_ref().or_else(|| self.runner.db_pool())
    }

    /// Retrieve a cloned SQLite database pool if configured.
    pub fn get_db_pool(&self) -> Option<sqlx::SqlitePool> {
        self.db_pool
            .clone()
            .or_else(|| self.runner.db_pool().cloned())
    }

    /// Create an isolated child dispatcher scoped to a delegated child agent, role, and task.
    pub fn create_child_dispatcher(
        &self,
        child_agent_id: crate::ids::AgentId,
        child_task_id: crate::ids::TaskId,
        child_role: crate::state_machine::agent::AgentRole,
        child_envelope: crate::agent::envelope::CapabilityEnvelope,
        child_depth: u32,
    ) -> Self {
        let child_context = self
            .context
            .clone()
            .with_agent_id(child_agent_id)
            .with_task_id(child_task_id)
            .with_agent_role(child_role)
            .with_role_envelope(child_envelope)
            .with_delegation_depth(child_depth);

        Self {
            runner: Arc::clone(&self.runner),
            context: child_context,
            policy_gate: Arc::clone(&self.policy_gate),
            autonomy_mode: self.autonomy_mode,
            replan_authority: self.replan_authority.clone(),
            session_repo: self.session_repo.clone(),
            session_id: self.session_id,
            event_bus: self.event_bus.clone(),
            db_pool: self.db_pool.clone(),
        }
    }
}

#[async_trait]
impl ActionDispatcher for ProductionActionDispatcher {
    async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
        let result = self
            .runner
            .execute_action(
                action,
                &self.context,
                self.policy_gate.as_ref(),
                self.autonomy_mode,
            )
            .await;
        Ok(result)
    }

    async fn dispatch_action(
        &self,
        action: &crate::agent::action::AgentAction,
    ) -> Result<crate::agent::action::AgentObservation, String> {
        let (tool_name, params) = match action {
            crate::agent::action::AgentAction::CallTool {
                tool_name,
                parameters,
            } => (tool_name.clone(), parameters.clone()),
            crate::agent::action::AgentAction::WriteChange { path, content, .. } => (
                "write_file".to_string(),
                serde_json::json!({ "path": path.display().to_string(), "content": content }),
            ),
            crate::agent::action::AgentAction::RunVerification { tier, target } => (
                "run_tests".to_string(),
                serde_json::json!({ "tier": tier, "target": target }),
            ),
            crate::agent::action::AgentAction::Complete { summary } => {
                return Ok(crate::agent::action::AgentObservation::Completed {
                    summary: summary.clone(),
                });
            }
            crate::agent::action::AgentAction::Fail { error } => {
                return Ok(crate::agent::action::AgentObservation::Failed {
                    error: error.clone(),
                    classification: None,
                });
            }
            crate::agent::action::AgentAction::Cancel { reason } => {
                return Ok(crate::agent::action::AgentObservation::Cancelled {
                    reason: reason.clone(),
                });
            }
            crate::agent::action::AgentAction::Replan {
                reason,
                tasks_to_supersede,
                new_tasks,
            } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "replan cancelled before execution".to_string(),
                    });
                }
                let mission_id = self.context.mission_id.ok_or_else(|| {
                    "replan execution failed: missing mission_id in execution context".to_string()
                })?;
                let authority = self.replan_authority.as_ref().ok_or_else(|| {
                    "replan execution failed: no replan authority configured on dispatcher"
                        .to_string()
                })?;
                let outcome = authority
                    .execute_replan_action(mission_id, reason, tasks_to_supersede, new_tasks)
                    .await
                    .map_err(|e| format!("replan authority execution failed: {e}"))?;
                return Ok(crate::agent::action::AgentObservation::ReplanCompleted {
                    new_plan_revision: outcome.revision,
                    superseded_count: outcome.superseded_tasks.len(),
                    added_count: outcome.new_tasks.len(),
                });
            }
            crate::agent::action::AgentAction::CreateTask {
                title,
                description,
                role,
                dependencies,
            } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "create_task cancelled before execution".to_string(),
                    });
                }
                let mission_id = self.context.mission_id.ok_or_else(|| {
                    "create_task execution failed: missing mission_id in execution context"
                        .to_string()
                })?;
                let pool = self.get_db_pool().ok_or_else(|| {
                    "create_task execution failed: missing database pool or task repository in execution context"
                        .to_string()
                })?;

                let graph_repo =
                    crate::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(pool);
                let task = graph_repo
                    .create_task_in_graph(
                        mission_id,
                        title,
                        Some(description.as_str()),
                        role.clone(),
                        dependencies,
                    )
                    .await
                    .map_err(|e| format!("create_task persistence failed: {e}"))?;

                if let Some(ref bus) = self.event_bus {
                    let event = crate::events::EventEnvelope::new(
                        0,
                        Some(mission_id),
                        None,
                        format!("agent:{}", self.context.agent_id.unwrap_or_default()),
                        crate::events::EventType::TaskCreated {
                            task_id: task.id,
                            mission_id,
                            title: task.title.clone(),
                            requirement_id: None,
                        },
                    );
                    let _ = bus.publish(event).await;
                }

                return Ok(crate::agent::action::AgentObservation::TaskCreated {
                    task_id: task.id,
                });
            }
            crate::agent::action::AgentAction::SupersedeTask { task_id, reason } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "supersede_task cancelled before execution".to_string(),
                    });
                }
                let mission_id = self.context.mission_id.ok_or_else(|| {
                    "supersede_task execution failed: missing mission_id in execution context"
                        .to_string()
                })?;
                let pool = self.get_db_pool().ok_or_else(|| {
                    "supersede_task execution failed: missing database pool or task repository in execution context"
                        .to_string()
                })?;

                let graph_repo =
                    crate::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(pool);
                let _task = graph_repo
                    .supersede_task_in_graph(mission_id, *task_id, reason)
                    .await
                    .map_err(|e| format!("supersede_task persistence failed: {e}"))?;

                if let Some(ref bus) = self.event_bus {
                    let event = crate::events::EventEnvelope::new(
                        0,
                        Some(mission_id),
                        None,
                        format!("agent:{}", self.context.agent_id.unwrap_or_default()),
                        crate::events::EventType::TaskSuperseded {
                            task_id: *task_id,
                            mission_id,
                            replacement_task_id: None,
                        },
                    );
                    let _ = bus.publish(event).await;
                }

                return Ok(crate::agent::action::AgentObservation::TaskSuperseded {
                    task_id: *task_id,
                });
            }
            crate::agent::action::AgentAction::RequestApproval {
                tool_name,
                parameters,
                risk,
                reason,
            } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "request_approval cancelled before resolution".to_string(),
                    });
                }
                if let Some(coordinator) = self.runner.approval_coordinator() {
                    let mission_id = self.context.mission_id.unwrap_or_default();
                    let risk_cls = match risk.as_deref().unwrap_or("").to_ascii_lowercase().as_str()
                    {
                        "read_only" | "readonly" => crate::tools::risk::RiskClass::ReadOnly,
                        "low" | "low_risk" | "low_risk_mutation" => {
                            crate::tools::risk::RiskClass::LowRiskMutation
                        }
                        "process" | "process_execution" => {
                            crate::tools::risk::RiskClass::ProcessExecution
                        }
                        "network" | "external" | "external_communication" => {
                            crate::tools::risk::RiskClass::ExternalCommunication
                        }
                        _ => crate::tools::risk::RiskClass::HighRiskMutation,
                    };
                    let req = crate::policy::approval::ApprovalRequest::new(
                        mission_id,
                        self.context.task_id,
                        None,
                        crate::ids::ToolCallId::new(),
                        tool_name.clone(),
                        parameters.clone(),
                        Vec::new(),
                        risk_cls,
                        None,
                        "pipeline-dispatch",
                        reason.clone(),
                    );
                    let timeout = std::time::Duration::from_secs(30);
                    // F-13: this execution's explicit wait setting travels
                    // with its context; never the shared global flag.
                    let wait_override = self.context.wait_for_approval;
                    match coordinator
                        .request_approval_with_options(
                            req,
                            self.autonomy_mode,
                            timeout,
                            wait_override,
                        )
                        .await
                    {
                        Ok(action) if action.is_allowed() => {
                            return Ok(crate::agent::action::AgentObservation::ApprovalResult {
                                approved: true,
                                reason: None,
                            });
                        }
                        Ok(crate::policy::approval::ApprovalAction::Deny { reason: r }) => {
                            return Ok(crate::agent::action::AgentObservation::ApprovalResult {
                                approved: false,
                                reason: Some(r),
                            });
                        }
                        Ok(crate::policy::approval::ApprovalAction::DenyAndCancelTask {
                            reason: r,
                        }) => {
                            return Ok(crate::agent::action::AgentObservation::Cancelled {
                                reason: r,
                            });
                        }
                        Ok(_) => {
                            return Ok(crate::agent::action::AgentObservation::ApprovalResult {
                                approved: false,
                                reason: Some("denied".to_string()),
                            });
                        }
                        Err(e) => {
                            return Err(format!("request_approval failed: {e}"));
                        }
                    }
                } else if self.autonomy_mode == crate::state::intake::AutonomyMode::Unattended {
                    return Ok(crate::agent::action::AgentObservation::ApprovalResult {
                        approved: false,
                        reason: Some("unattended mode denied privileged operation without approval coordinator".to_string()),
                    });
                } else {
                    return Err(
                        "request_approval failed: no approval coordinator configured on dispatcher"
                            .to_string(),
                    );
                }
            }
            crate::agent::action::AgentAction::AskUser { question, options } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "ask_user cancelled before response".to_string(),
                    });
                }

                if self.autonomy_mode == AutonomyMode::Unattended
                    || self.autonomy_mode == AutonomyMode::Plan
                {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "Unattended mode strictly denies interactive ask_user requests"
                            .to_string(),
                    });
                }

                let pool_opt = self.runner.db_pool().cloned();
                let session_repo = self.session_repo.clone().or_else(|| {
                    pool_opt.as_ref().map(|p| {
                        crate::interaction::session::SqliteSessionRepository::new(p.clone())
                    })
                });

                let Some(session_repo) = session_repo else {
                    return Err(
                        "ask_user failed: no session repository or database pool configured on dispatcher"
                            .to_string(),
                    );
                };

                let session_id = if let Some(sid) = self.session_id {
                    sid
                } else if let Some(ref pool) = pool_opt {
                    if let Some(mid) = self.context.mission_id {
                        let row_opt: Option<(Vec<u8>,)> = sqlx::query_as(
                            "SELECT id FROM sessions WHERE active_mission_id = ? OR active_run_id = ? OR mission_id = ? ORDER BY updated_at DESC LIMIT 1",
                        )
                        .bind(mid.as_bytes().as_slice())
                        .bind(mid.as_bytes().as_slice())
                        .bind(mid.as_bytes().as_slice())
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);
                        if let Some((bytes,)) = row_opt {
                            if bytes.len() == 16 {
                                let mut arr = [0u8; 16];
                                arr.copy_from_slice(&bytes);
                                crate::ids::SessionId::from_bytes(arr)
                            } else {
                                return Err(
                                    "ask_user failed: corrupt session id in database".to_string()
                                );
                            }
                        } else {
                            let row_opt: Option<(Vec<u8>,)> = sqlx::query_as(
                                "SELECT id FROM sessions ORDER BY updated_at DESC LIMIT 1",
                            )
                            .fetch_optional(pool)
                            .await
                            .unwrap_or(None);
                            if let Some((bytes,)) = row_opt {
                                if bytes.len() == 16 {
                                    let mut arr = [0u8; 16];
                                    arr.copy_from_slice(&bytes);
                                    crate::ids::SessionId::from_bytes(arr)
                                } else {
                                    return Err("ask_user failed: corrupt session id in database"
                                        .to_string());
                                }
                            } else {
                                return Err(
                                    "ask_user failed: no active session found to persist pending question"
                                        .to_string(),
                                );
                            }
                        }
                    } else {
                        let row_opt: Option<(Vec<u8>,)> = sqlx::query_as(
                            "SELECT id FROM sessions ORDER BY updated_at DESC LIMIT 1",
                        )
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);
                        if let Some((bytes,)) = row_opt {
                            if bytes.len() == 16 {
                                let mut arr = [0u8; 16];
                                arr.copy_from_slice(&bytes);
                                crate::ids::SessionId::from_bytes(arr)
                            } else {
                                return Err(
                                    "ask_user failed: corrupt session id in database".to_string()
                                );
                            }
                        } else {
                            return Err(
                                "ask_user failed: no active session found to persist pending question"
                                    .to_string(),
                            );
                        }
                    }
                } else {
                    return Err(
                        "ask_user failed: cannot resolve session id without database pool"
                            .to_string(),
                    );
                };

                let q_id = format!(
                    "ask_user_{}_{}",
                    self.context
                        .task_id
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "task".to_string()),
                    uuid::Uuid::now_v7()
                );

                let decision = crate::interaction::decision::InteractiveDecision::ask_user(
                    session_id.to_string(),
                    self.context.mission_id.map(|m| m.to_string()),
                    q_id.clone(),
                    question.clone(),
                    options,
                );

                // Persist pending decision first: fail closed if persistence fails
                session_repo
                    .set_pending_decision(session_id, &decision)
                    .await
                    .map_err(|e| format!("ask_user persistence failure: {e}"))?;

                // Emit interaction event only after successful persistence
                if let Some(ref bus) = self.event_bus {
                    let env = crate::events::EventEnvelope::new(
                        0,
                        self.context.mission_id,
                        None,
                        "pipeline_dispatcher".to_string(),
                        crate::events::EventType::OperatorEscalationRequested {
                            request_id: q_id.clone(),
                            mission_id: self.context.mission_id.unwrap_or_default(),
                            reason: question.clone(),
                            timeout_seconds: Some(300),
                            tool_name: Some("ask_user".to_string()),
                            parameters_summary: Some(
                                serde_json::to_string(options).unwrap_or_default(),
                            ),
                            risk_tier: Some("Interactive".to_string()),
                            agent_role: self.context.agent_id.as_ref().map(|a| a.to_string()),
                        },
                    );
                    let _ = bus.publish(env).await;
                }

                // Register waiter channel
                let rx = crate::interaction::decision::DecisionWaiterRegistry::global()
                    .register(q_id.clone())
                    .await;

                // Suspend task execution until actual answer, cancellation, or timeout
                let timeout_dur = std::time::Duration::from_secs(300);
                return tokio::select! {
                    _ = self.context.cancellation_token.cancelled() => {
                        crate::interaction::decision::DecisionWaiterRegistry::global().remove(&q_id).await;
                        let _ = session_repo.clear_pending_decision(session_id).await;
                        Ok(crate::agent::action::AgentObservation::Cancelled {
                            reason: "ask_user cancelled while awaiting response".to_string(),
                        })
                    }
                    outcome_res = rx => {
                        match outcome_res {
                            Ok(crate::interaction::decision::DecisionResumeOutcome::Answer(answer)) => {
                                Ok(crate::agent::action::AgentObservation::UserResponse { answer })
                            }
                            Ok(crate::interaction::decision::DecisionResumeOutcome::Cancelled(reason)) => {
                                Ok(crate::agent::action::AgentObservation::Cancelled { reason })
                            }
                            Ok(crate::interaction::decision::DecisionResumeOutcome::Rejected(reason)) => {
                                Ok(crate::agent::action::AgentObservation::Cancelled { reason })
                            }
                            Err(_) => {
                                Err("ask_user waiter channel dropped without answer".to_string())
                            }
                        }
                    }
                    _ = tokio::time::sleep(timeout_dur) => {
                        crate::interaction::decision::DecisionWaiterRegistry::global().remove(&q_id).await;
                        let _ = session_repo.clear_pending_decision(session_id).await;
                        Err("ask_user timed out awaiting user response".to_string())
                    }
                };
            }
            crate::agent::action::AgentAction::Delegate {
                target_role,
                task_id: _,
                reason,
                inputs: _,
            } => {
                let req = ActionRequest {
                    id: format!("act-{}", uuid::Uuid::now_v7()),
                    tool_name: "delegate_task".to_string(),
                    parameters: serde_json::json!({
                        "target_role": target_role.to_string(),
                        "subtask": reason,
                    }),
                };
                let res = self.dispatch(&req).await?;
                if res.success {
                    return Ok(crate::agent::action::AgentObservation::DelegationResult {
                        child_agent_id: crate::ids::AgentId::new(),
                        outcome: Box::new(crate::agent::supervisor::AgentOutcome::Succeeded {
                            output: res.output,
                            steps_consumed: 1,
                        }),
                    });
                } else {
                    return Ok(crate::agent::action::AgentObservation::DelegationResult {
                        child_agent_id: crate::ids::AgentId::new(),
                        outcome: Box::new(crate::agent::supervisor::AgentOutcome::Failed(
                            crate::agent::supervisor::FailureEvidence {
                                failure_class: crate::agent::supervisor::FailureClass::ToolFailure,
                                message: res.error.unwrap_or(res.output),
                                step_number: 1,
                                occurred_at: chrono::Utc::now(),
                                is_panic: false,
                                diagnostics: std::collections::HashMap::new(),
                            },
                        )),
                    });
                }
            }
            crate::agent::action::AgentAction::Pause { .. } => {
                return Ok(crate::agent::action::AgentObservation::Paused);
            }
            crate::agent::action::AgentAction::Resume { .. } => {
                return Ok(crate::agent::action::AgentObservation::Resumed);
            }
        };

        let req = ActionRequest {
            id: format!("act-{}", uuid::Uuid::now_v7()),
            tool_name: tool_name.clone(),
            parameters: params,
        };
        let start = std::time::Instant::now();
        let res = self.dispatch(&req).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(crate::agent::action::AgentObservation::ToolOutput {
            tool_name,
            call_id: res.action_id,
            output: if res.success {
                res.output
            } else {
                res.error.unwrap_or(res.output)
            },
            success: res.success,
            duration_ms,
        })
    }

    fn create_child_dispatcher(
        &self,
        child_agent_id: crate::ids::AgentId,
        child_task_id: crate::ids::TaskId,
        child_role: crate::state_machine::agent::AgentRole,
        child_envelope: crate::agent::envelope::CapabilityEnvelope,
        child_depth: u32,
    ) -> Arc<dyn ActionDispatcher> {
        Arc::new(self.create_child_dispatcher(
            child_agent_id,
            child_task_id,
            child_role,
            child_envelope,
            child_depth,
        ))
    }

    fn permitted_tool_schemas(
        &self,
        role: &crate::state_machine::agent::AgentRole,
    ) -> Vec<serde_json::Value> {
        if !crate::agent::registry::RoleRegistry::write_tools_permitted_for(role) {
            self.runner
                .tool_registry()
                .list_tools()
                .iter()
                .filter(|t| {
                    !crate::verification::gate::ToolCapabilityClass::classify(t.id()).is_mutation()
                })
                .map(|t| crate::tools::definition::to_openai_tool(t.as_ref()))
                .collect()
        } else {
            self.runner
                .tool_registry()
                .list_tools()
                .iter()
                .map(|t| crate::tools::definition::to_openai_tool(t.as_ref()))
                .collect()
        }
    }
}
