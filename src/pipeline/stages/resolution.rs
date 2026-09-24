//! Stage 1: Tool Resolution Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::tools::definition::AnyTool;
use crate::tools::registry::ToolRegistry;

/// Typed state envelope proving tool resolution succeeded.
pub struct ResolvedToolState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
}

/// Stage 1: Resolves requested `tool_name` against the central `ToolRegistry`.
pub struct ToolResolutionStage;

impl ToolResolutionStage {
    pub fn execute(
        action: &ActionRequest,
        registry: &Arc<ToolRegistry>,
    ) -> Result<ResolvedToolState, ToolError> {
        let tool = registry.get(&action.tool_name).ok_or_else(|| {
            ToolError::not_found(
                "TOOL_NOT_FOUND",
                format!(
                    "Tool '{}' is not registered in ToolRegistry",
                    action.tool_name
                ),
                Some("Verify the tool name against model-visible registered tools".to_string()),
            )
            .with_provenance("ToolResolutionStage")
        })?;

        Ok(ResolvedToolState {
            action: action.clone(),
            tool,
        })
    }
}
