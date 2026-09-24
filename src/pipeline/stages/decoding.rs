//! Stage 2: Argument Decoding Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::resolution::ResolvedToolState;
use crate::tools::definition::AnyTool;

/// Typed state envelope proving raw parameter arguments were decoded.
pub struct DecodedArgsState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
}

/// Stage 2: Parses raw parameters and ensures they represent decodable parameter input.
pub struct ArgDecodingStage;

impl ArgDecodingStage {
    pub fn execute(state: ResolvedToolState) -> Result<DecodedArgsState, ToolError> {
        let params = state.action.parameters.clone();

        // 1. Check for malformed / truncated syntax envelope from wire protocol (P3-B, MDL-01)
        if let Some(err_msg) = params.get("__malformed_error__").and_then(|v| v.as_str()) {
            let raw = params
                .get("__raw_arguments__")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_length =
                params.get("__finish_reason__").and_then(|v| v.as_str()) == Some("length");

            let hint = if is_length {
                "Tool call arguments were cut off by token limit. Break operations into smaller steps (e.g. edit a smaller line range) or provide shorter arguments."
            } else {
                "Ensure tool arguments are valid, well-formed JSON matching the parameter schema. Check for unescaped quotes or invalid formatting."
            };

            return Err(ToolError::validation(
                if is_length {
                    "ARG_TRUNCATED"
                } else {
                    "ARG_SYNTAX_ERROR"
                },
                format!(
                    "Malformed arguments in tool call '{}': {}. Raw snippet: {}",
                    state.tool.id(),
                    err_msg,
                    raw
                ),
                Some(hint.to_string()),
            )
            .with_provenance("ArgDecodingStage"));
        }

        // 2. Parameter arguments must be a JSON object (or empty/null for parameterless tools)
        if !params.is_object() && !params.is_null() {
            return Err(ToolError::validation(
                "ARG_DECODING_FAILED",
                format!("Tool parameters must be a JSON object, received {}", params),
                Some(
                    "Pass arguments as a valid JSON object matching the tool's schema".to_string(),
                ),
            )
            .with_provenance("ArgDecodingStage"));
        }

        let decoded = if params.is_null() {
            serde_json::json!({})
        } else {
            params
        };

        Ok(DecodedArgsState {
            action: state.action,
            tool: state.tool,
            decoded_args: decoded,
        })
    }
}
