//! Production action dispatcher wiring WorkerRunner to the 11-stage pipeline (TL-03, D-12).

use async_trait::async_trait;
use std::sync::Arc;

use crate::agent::runner::{ActionDispatcher, ActionRequest, ActionResult};
use crate::kernel::seams::policy::PolicyGate;
use crate::pipeline::runner::ToolPipelineRunner;
use crate::state::intake::AutonomyMode;
use crate::tools::definition::ToolExecutionContext;

/// Production action dispatcher executing actions through the non-bypassable 11-stage pipeline (TL-03).
pub struct ProductionActionDispatcher {
    runner: Arc<ToolPipelineRunner>,
    context: ToolExecutionContext,
    policy_gate: Arc<dyn PolicyGate>,
    autonomy_mode: AutonomyMode,
}

impl ProductionActionDispatcher {
    /// Create a new production action dispatcher.
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
        }
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
}
