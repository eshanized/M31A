//! Structured tool output schema and result validation.
//!
//! Provides immutable shared storage for expected tool result schemas and validates
//! tool return payloads against typed JSON Schema specifications.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum OutputValidationError {
    #[error("Missing required property '{0}' in tool output")]
    MissingProperty(String),
    #[error("Type mismatch for property '{property}': expected {expected}, found {found}")]
    TypeMismatch {
        property: String,
        expected: String,
        found: String,
    },
    #[error("Invalid root schema: {0}")]
    InvalidSchema(String),
}

/// Immutable output schema for validating tool responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutputSchema(OutputSchemaStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputSchemaStorage {
    Json(Arc<Value>),
    McpStructuredContent(Option<Arc<Map<String, Value>>>),
}

impl Serialize for ToolOutputSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolOutputSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = Value::deserialize(deserializer)?;
        Ok(Self::from_json_schema(val))
    }
}

impl ToolOutputSchema {
    pub fn from_json_schema(val: Value) -> Self {
        Self(OutputSchemaStorage::Json(Arc::new(val)))
    }

    pub fn from_mcp_output_schema(structured_content: Option<Map<String, Value>>) -> Self {
        Self(OutputSchemaStorage::McpStructuredContent(
            structured_content.map(Arc::new),
        ))
    }

    /// Materialize the complete JSON schema value.
    pub fn to_value(&self) -> Value {
        match &self.0 {
            OutputSchemaStorage::Json(v) => v.as_ref().clone(),
            OutputSchemaStorage::McpStructuredContent(content) => {
                let mut map = Map::new();
                map.insert("type".to_string(), Value::String("object".to_string()));
                if let Some(c) = content {
                    map.insert("properties".to_string(), Value::Object(c.as_ref().clone()));
                }
                Value::Object(map)
            }
        }
    }

    /// Consume into owned JSON schema value.
    pub fn into_value(self) -> Value {
        match self.0 {
            OutputSchemaStorage::Json(v) => Arc::unwrap_or_clone(v),
            OutputSchemaStorage::McpStructuredContent(content) => {
                let mut map = Map::new();
                map.insert("type".to_string(), Value::String("object".to_string()));
                if let Some(c) = content {
                    map.insert(
                        "properties".to_string(),
                        Value::Object(Arc::unwrap_or_clone(c)),
                    );
                }
                Value::Object(map)
            }
        }
    }

    /// Validate a tool output JSON payload against this schema.
    pub fn validate(&self, output: &Value) -> Result<(), OutputValidationError> {
        let schema_val = self.to_value();
        validate_json_value(output, &schema_val, "")
    }
}

impl From<Value> for ToolOutputSchema {
    fn from(val: Value) -> Self {
        Self::from_json_schema(val)
    }
}

fn validate_json_value(
    val: &Value,
    schema: &Value,
    path: &str,
) -> Result<(), OutputValidationError> {
    if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
        let actual_type = match val {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };

        if expected_type != actual_type && !(expected_type == "integer" && val.is_i64()) {
            return Err(OutputValidationError::TypeMismatch {
                property: if path.is_empty() {
                    "root".to_string()
                } else {
                    path.to_string()
                },
                expected: expected_type.to_string(),
                found: actual_type.to_string(),
            });
        }
    }

    if let (Value::Object(val_map), Some(Value::Object(props))) = (val, schema.get("properties")) {
        // Check required fields
        if let Some(Value::Array(reqs)) = schema.get("required") {
            for req in reqs {
                if let Some(field) = req.as_str()
                    && !val_map.contains_key(field)
                {
                    return Err(OutputValidationError::MissingProperty(format!(
                        "{}{}",
                        if path.is_empty() { "" } else { "." },
                        field
                    )));
                }
            }
        }

        // Check property sub-schemas
        for (k, v) in val_map {
            if let Some(prop_schema) = props.get(k) {
                let sub_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                validate_json_value(v, prop_schema, &sub_path)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_output_schema_validation_success() {
        let schema_json = json!({
            "type": "object",
            "properties": {
                "status": { "type": "string" },
                "count": { "type": "number" },
                "success": { "type": "boolean" }
            },
            "required": ["status", "success"]
        });

        let schema = ToolOutputSchema::from(schema_json);

        let valid_payload = json!({
            "status": "completed",
            "count": 42,
            "success": true
        });

        assert_eq!(schema.validate(&valid_payload), Ok(()));
    }

    #[test]
    fn test_tool_output_schema_validation_missing_property() {
        let schema_json = json!({
            "type": "object",
            "properties": {
                "status": { "type": "string" },
                "id": { "type": "string" }
            },
            "required": ["status", "id"]
        });

        let schema = ToolOutputSchema::from(schema_json);

        let invalid_payload = json!({
            "status": "pending"
        });

        let res = schema.validate(&invalid_payload);
        assert_eq!(
            res,
            Err(OutputValidationError::MissingProperty("id".to_string()))
        );
    }

    #[test]
    fn test_tool_output_schema_validation_type_mismatch() {
        let schema_json = json!({
            "type": "object",
            "properties": {
                "age": { "type": "number" }
            }
        });

        let schema = ToolOutputSchema::from(schema_json);

        let invalid_payload = json!({
            "age": "forty-two"
        });

        let res = schema.validate(&invalid_payload);
        assert!(matches!(
            res,
            Err(OutputValidationError::TypeMismatch { .. })
        ));
    }

    #[test]
    fn test_mcp_output_schema_envelope() {
        let mut props = Map::new();
        props.insert("diff".to_string(), json!({ "type": "string" }));

        let mcp_schema = ToolOutputSchema::from_mcp_output_schema(Some(props));
        let val = mcp_schema.to_value();
        assert_eq!(val["type"], "object");
        assert!(val["properties"]["diff"].is_object());
    }
}
