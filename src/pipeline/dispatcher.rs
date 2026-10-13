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
        }
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
                dependencies: _,
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
                let task_repo = self.context.task_repo.as_ref().ok_or_else(|| {
                    "create_task execution failed: missing task_repo in execution context"
                        .to_string()
                })?;
                if title.trim().is_empty() {
                    return Err("create_task execution failed: title cannot be empty".to_string());
                }
                let task_id = crate::ids::TaskId::new();
                let mut task = crate::state::Task::new(task_id, mission_id, title.clone());
                task.description = Some(description.clone());
                if let Some(r) = role {
                    task.role = r.clone();
                }
                task_repo
                    .insert(&task)
                    .await
                    .map_err(|e| format!("create_task persistence failed: {e}"))?;
                return Ok(crate::agent::action::AgentObservation::TaskCreated { task_id });
            }
            crate::agent::action::AgentAction::SupersedeTask { task_id, reason } => {
                if self.context.cancellation_token.is_cancelled() {
                    return Ok(crate::agent::action::AgentObservation::Cancelled {
                        reason: "supersede_task cancelled before execution".to_string(),
                    });
                }
                let task_repo = self.context.task_repo.as_ref().ok_or_else(|| {
                    "supersede_task execution failed: missing task_repo in execution context"
                        .to_string()
                })?;
                if reason.trim().is_empty() {
                    return Err(
                        "supersede_task execution failed: reason cannot be empty".to_string()
                    );
                }
                task_repo
                    .mark_cancelled(*task_id, chrono::Utc::now())
                    .await
                    .map_err(|e| format!("supersede_task persistence failed: {e}"))?;
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
                    match coordinator
                        .request_approval(req, self.autonomy_mode, timeout)
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
                if let Some(coordinator) = self.runner.approval_coordinator() {
                    let mission_id = self.context.mission_id.unwrap_or_default();
                    let req = crate::policy::approval::ApprovalRequest::new(
                        mission_id,
                        self.context.task_id,
                        None,
                        crate::ids::ToolCallId::new(),
                        "ask_user".to_string(),
                        serde_json::json!({ "question": question, "options": options }),
                        Vec::new(),
                        crate::tools::risk::RiskClass::ReadOnly,
                        None,
                        "pipeline-dispatch",
                        question.clone(),
                    );
                    let timeout = std::time::Duration::from_secs(300);
                    match coordinator
                        .request_approval(req, self.autonomy_mode, timeout)
                        .await
                    {
                        Ok(action) if action.is_allowed() => {
                            let answer = options
                                .first()
                                .map(|o| o.label.clone())
                                .unwrap_or_else(|| "approved".to_string());
                            return Ok(crate::agent::action::AgentObservation::UserResponse {
                                answer,
                            });
                        }
                        Ok(crate::policy::approval::ApprovalAction::Deny { reason: r }) => {
                            return Ok(crate::agent::action::AgentObservation::UserResponse {
                                answer: r,
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
                            return Ok(crate::agent::action::AgentObservation::Cancelled {
                                reason: "ask_user cancelled".to_string(),
                            });
                        }
                        Err(e) => {
                            return Err(format!("ask_user failed: {e}"));
                        }
                    }
                } else {
                    return Err(
                        "ask_user failed: no interactive coordinator configured on dispatcher"
                            .to_string(),
                    );
                }
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
}
