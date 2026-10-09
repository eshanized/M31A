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
            other => {
                return Err(format!(
                    "action {} not supported by production dispatcher",
                    other.name()
                ));
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
