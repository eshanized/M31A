//! Stage 11: Telemetry & Durable Record Stage (TL-03, per D-09, D-12).

use std::time::Duration;

use crate::agent::runner::ActionResult;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::capture_normalization::NormalizedResultState;

/// Stage 11: Emits structured tracing spans and produces the final `ActionResult`.
pub struct TelemetryDurableRecordStage;

impl TelemetryDurableRecordStage {
    pub fn execute(state: NormalizedResultState) -> ActionResult {
        let err_category = state.error.as_ref().map(|e| e.category.as_str());
        let err_code = state.error.as_ref().map(|e| e.code.as_str());

        if state.success {
            tracing::info!(
                action_id = %state.action_id,
                tool_id = %state.tool_id,
                duration_ms = state.duration.as_millis(),
                output_bytes = state.evidence.model_visible_output.len(),
                audit_digest = %state.evidence.audit_digest,
                "Tool execution pipeline completed successfully"
            );
        } else {
            tracing::warn!(
                action_id = %state.action_id,
                tool_id = %state.tool_id,
                duration_ms = state.duration.as_millis(),
                error_category = ?err_category,
                error_code = ?err_code,
                audit_digest = %state.evidence.audit_digest,
                "Tool execution pipeline finished with error or denial"
            );
        }

        ActionResult {
            action_id: state.action_id,
            success: state.success,
            output: state.output,
            error: state.error.map(|e| e.to_model_diagnostic()),
        }
    }

    /// Convenience helper for recording an early pipeline abort before Stage 9.
    pub fn record_failure(action_id: String, tool_id: String, error: ToolError) -> ActionResult {
        let normalized = NormalizedResultState {
            action_id,
            tool_id,
            success: false,
            output: String::new(),
            error: Some(error),
            duration: Duration::ZERO,
            evidence: crate::pipeline::stages::capture_normalization::PipelineOutputEvidence::empty(
            ),
        };
        Self::execute(normalized)
    }
}
