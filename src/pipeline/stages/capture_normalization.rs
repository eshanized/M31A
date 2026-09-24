//! Stage 10: Capture & Normalization Stage (TL-03, per D-09, D-11).

use std::sync::Arc;
use std::time::Duration;

use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::pipeline::capture::OutputCaptureManager;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::execution::RawExecutionState;

/// Typed state envelope containing normalized result.
pub struct NormalizedResultState {
    pub action_id: String,
    pub tool_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<ToolError>,
    pub duration: Duration,
}

/// Stage 10: Captures output, applies budget-aware artifact externalization, and normalizes outcome.
pub struct CaptureNormalizationStage;

impl CaptureNormalizationStage {
    pub async fn execute(
        state: RawExecutionState,
        capture_manager: &OutputCaptureManager,
        artifact_store: Option<&Arc<dyn ArtifactStore>>,
    ) -> NormalizedResultState {
        let action_id = state.action.id;
        let tool_id = state.tool_id;
        let duration = state.duration;

        match state.outcome {
            Ok(value) => {
                // Extract clean text representation of output
                let raw_output = if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
                };

                // Apply budget ceilings: inline vs external artifact
                match capture_manager
                    .process_output(&raw_output, artifact_store)
                    .await
                {
                    Ok(processed_output) => NormalizedResultState {
                        action_id,
                        tool_id,
                        success: true,
                        output: processed_output,
                        error: None,
                        duration,
                    },
                    Err(artifact_err) => {
                        // Fail closed on artifactization failure (D-11)
                        NormalizedResultState {
                            action_id,
                            tool_id,
                            success: false,
                            output: String::new(),
                            error: Some(artifact_err),
                            duration,
                        }
                    }
                }
            }
            Err(tool_err) => NormalizedResultState {
                action_id,
                tool_id,
                success: false,
                output: String::new(),
                error: Some(tool_err),
                duration,
            },
        }
    }
}
