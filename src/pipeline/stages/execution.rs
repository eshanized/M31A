//! Stage 9: Tool Execution Stage (TL-03, per D-09).
//!
//! Non-negotiable security invariant: Side-effects occur STRICTLY in this stage.
//! Stages 1–8 enforce structural, semantic, capability, resource, and policy authorization without mutations.

use std::time::{Duration, Instant};

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::approval_resolution::ExecutionAuthorizedState;
use crate::tools::definition::ToolExecutionContext;

/// Typed state envelope containing raw execution outcome.
pub struct RawExecutionState {
    pub action: ActionRequest,
    pub tool_id: String,
    pub outcome: Result<serde_json::Value, ToolError>,
    pub duration: Duration,
}

/// Stage 9: The ONLY stage in the pipeline where physical side effects occur.
pub struct ToolExecutionStage;

impl ToolExecutionStage {
    pub async fn execute(
        state: ExecutionAuthorizedState,
        context: &ToolExecutionContext,
    ) -> RawExecutionState {
        let tool_id = state.tool.id().to_string();
        let start = Instant::now();

        // F-09 race guard: re-resolve every authorized binding against the
        // LIVE filesystem immediately before the side effect. A symlink swap
        // (or ancestor replacement) between authorization and execution is
        // refused here — the mutation never runs against a target the
        // operator did not authorize.
        if let Err(binding_err) =
            crate::pipeline::stages::resource_scope::ResourceScopeStage::verify_bindings_live(
                &state.bound_resources,
                &context.workspace_root,
            )
        {
            let duration = start.elapsed();
            return RawExecutionState {
                action: state.action,
                tool_id,
                outcome: Err(binding_err),
                duration,
            };
        }

        // Physical side effect execution
        let outcome = state
            .tool
            .execute_raw(context, state.decoded_args)
            .await
            .map_err(ToolError::from);

        let duration = start.elapsed();

        RawExecutionState {
            action: state.action,
            tool_id,
            outcome,
            duration,
        }
    }
}
