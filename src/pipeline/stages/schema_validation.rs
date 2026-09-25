//! Stage 3: Schema Validation Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::decoding::DecodedArgsState;
use crate::tools::definition::AnyTool;

/// Typed state envelope proving schema validation succeeded.
pub struct SchemaValidatedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
}

/// Stage 3: Validates parameter structure against tool's derived JSON schema.
pub struct SchemaValidationStage;

impl SchemaValidationStage {
    pub fn execute(state: DecodedArgsState) -> Result<SchemaValidatedState, ToolError> {
        let schema = state.tool.parameter_schema();
        let mut decoded_args = state.decoded_args;

        // 1. Normalize common parameter aliases across model proposals (F08, P3-B)
        if let Some(args_obj) = decoded_args.as_object_mut() {
            if !args_obj.contains_key("path")
                && let Some(v) = args_obj
                    .get("file_path")
                    .or_else(|| args_obj.get("file"))
                    .or_else(|| args_obj.get("filename"))
                    .or_else(|| args_obj.get("target"))
            {
                let v_clone = v.clone();
                args_obj.insert("path".to_string(), v_clone);
            }
            if !args_obj.contains_key("target")
                && let Some(v) = args_obj
                    .get("path")
                    .or_else(|| args_obj.get("file_path"))
                    .or_else(|| args_obj.get("file"))
            {
                let v_clone = v.clone();
                args_obj.insert("target".to_string(), v_clone);
            }
        }

        let args = &decoded_args;

        // 2. If schema declares required properties, check each required property exists and is not null
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for req in required {
                if let Some(req_name) = req.as_str() {
                    let val = args.get(req_name);
                    if val.is_none() || val == Some(&serde_json::Value::Null) {
                        return Err(ToolError::validation(
                            "SCHEMA_VALIDATION_FAILED",
                            format!(
                                "Tool '{}' missing required parameter property '{}'",
                                state.tool.id(),
                                req_name
                            ),
                            Some(format!(
                                "Provide required parameter property '{}' according to tool schema",
                                req_name
                            )),
                        )
                        .with_provenance("SchemaValidationStage"));
                    }
                }
            }
        }

        // 3. If schema declares properties, validate and coerce basic types for supplied properties
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object())
            && let Some(args_obj) = decoded_args.as_object_mut()
        {
            for (key, val) in args_obj.iter_mut() {
                if let Some(prop_schema) = properties.get(key) {
                    coerce_value_to_schema_type(val, prop_schema);

                    if !matches_schema_type(val, prop_schema) {
                        return Err(ToolError::validation(
                            "SCHEMA_TYPE_MISMATCH",
                            format!(
                                "Parameter property '{}' has invalid type in call to '{}'",
                                key,
                                state.tool.id()
                            ),
                            Some(format!(
                                "Ensure parameter '{}' matches type defined in schema: {:?}",
                                key,
                                prop_schema.get("type")
                            )),
                        )
                        .with_provenance("SchemaValidationStage"));
                    }
                }
            }
        }

        Ok(SchemaValidatedState {
            action: state.action,
            tool: state.tool,
            decoded_args,
        })
    }
}

/// Helper to coerce stringified numbers and booleans into declared schema types.
fn coerce_value_to_schema_type(val: &mut serde_json::Value, schema: &serde_json::Value) {
    let target_type = if let Some(t) = schema.get("type").and_then(|t| t.as_str()) {
        t
    } else if let Some(types) = schema.get("type").and_then(|t| t.as_array()) {
        if types.iter().any(|t| t.as_str() == Some("integer")) {
            "integer"
        } else if types.iter().any(|t| t.as_str() == Some("number")) {
            "number"
        } else if types.iter().any(|t| t.as_str() == Some("boolean")) {
            "boolean"
        } else {
            return;
        }
    } else {
        return;
    };

    if let serde_json::Value::String(s) = val {
        let trimmed = s.trim();
        match target_type {
            "integer" => {
                if let Ok(n) = trimmed.parse::<i64>() {
                    *val = serde_json::Value::Number(n.into());
                }
            }
            "number" => {
                if let Ok(f) = trimmed.parse::<f64>()
                    && let Some(num) = serde_json::Number::from_f64(f)
                {
                    *val = serde_json::Value::Number(num);
                }
            }
            "boolean" => {
                if trimmed.eq_ignore_ascii_case("true") {
                    *val = serde_json::Value::Bool(true);
                } else if trimmed.eq_ignore_ascii_case("false") {
                    *val = serde_json::Value::Bool(false);
                }
            }
            _ => {}
        }
    }
}

/// Helper to check if a JSON value matches the declared schema type.
fn matches_schema_type(val: &serde_json::Value, schema: &serde_json::Value) -> bool {
    // If value is null, it's allowed if schema allows null or type is an array containing "null"
    if val.is_null() {
        if let Some(types) = schema.get("type").and_then(|t| t.as_array()) {
            return types.iter().any(|t| t.as_str() == Some("null"));
        }
        return false;
    }

    if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
        return check_single_type(val, type_str);
    }

    if let Some(types) = schema.get("type").and_then(|t| t.as_array()) {
        return types.iter().any(|t| {
            t.as_str()
                .map(|ts| check_single_type(val, ts))
                .unwrap_or(false)
        });
    }

    // If no type specified or complex schema ($ref, anyOf, etc.), pass basic structural check
    true
}

fn check_single_type(val: &serde_json::Value, expected: &str) -> bool {
    match expected {
        "string" => val.is_string(),
        "integer" => {
            val.is_i64()
                || val.is_u64()
                || (val.is_string() && val.as_str().unwrap().trim().parse::<i64>().is_ok())
        }
        "number" => {
            val.is_number()
                || (val.is_string() && val.as_str().unwrap().trim().parse::<f64>().is_ok())
        }
        "boolean" => {
            val.is_boolean()
                || (val.is_string()
                    && matches!(
                        val.as_str().unwrap().trim().to_ascii_lowercase().as_str(),
                        "true" | "false"
                    ))
        }
        // LLMs frequently pass arrays either as native JSON arrays or stringified sequences (e.g. "['a', 'b']" or "--flag arg")
        "array" => val.is_array() || val.is_string(),
        "object" => val.is_object(),
        "null" => val.is_null(),
        _ => true,
    }
}
