//! Stage 4: Semantic Validation Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::schema_validation::SchemaValidatedState;
use crate::tools::definition::AnyTool;

/// Typed state envelope proving semantic validation succeeded.
pub struct SemanticallyValidatedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
}

/// Stage 4: Validates parameters semantically (e.g. non-empty required strings, valid bounds).
pub struct SemanticValidationStage;

impl SemanticValidationStage {
    pub fn execute(state: SchemaValidatedState) -> Result<SemanticallyValidatedState, ToolError> {
        let args = &state.decoded_args;

        if let Some(args_obj) = args.as_object() {
            for (key, val) in args_obj {
                // Semantic check 1: Target identifiers, commands, or file paths must not be empty or whitespace-only
                if matches!(
                    key.as_str(),
                    "path"
                        | "file_path"
                        | "destination"
                        | "target"
                        | "command"
                        | "query"
                        | "pattern"
                        | "tool_name"
                ) && let Some(s) = val.as_str()
                    && s.trim().is_empty()
                {
                    return Err(ToolError::validation(
                        "SEMANTIC_VALIDATION_EMPTY_FIELD",
                        format!(
                            "Parameter '{}' cannot be empty or blank whitespace in call to '{}'",
                            key,
                            state.tool.id()
                        ),
                        Some(format!("Provide a non-empty value for '{}'", key)),
                    )
                    .with_provenance("SemanticValidationStage"));
                }

                // Semantic check 2: Numerical limits or counts must be reasonable
                if matches!(
                    key.as_str(),
                    "limit" | "max_results" | "repeat_count" | "depth"
                ) && let Some(n) = val.as_i64()
                {
                    if n < 0 {
                        return Err(ToolError::validation(
                            "SEMANTIC_VALIDATION_INVALID_BOUNDS",
                            format!(
                                "Parameter '{}' must be non-negative in call to '{}'",
                                key,
                                state.tool.id()
                            ),
                            Some(format!("Specify a non-negative value for '{}'", key)),
                        )
                        .with_provenance("SemanticValidationStage"));
                    }
                    if n > 1_000_000 {
                        return Err(ToolError::validation(
                            "SEMANTIC_VALIDATION_BOUNDS_EXCEEDED",
                            format!(
                                "Parameter '{}' exceeds safety maximum limit in call to '{}'",
                                key,
                                state.tool.id()
                            ),
                            Some(format!("Limit '{}' to 1,000,000 or fewer", key)),
                        )
                        .with_provenance("SemanticValidationStage"));
                    }
                }
            }
        }

        Ok(SemanticallyValidatedState {
            action: state.action,
            tool: state.tool,
            decoded_args: state.decoded_args,
        })
    }
}
