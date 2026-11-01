//! Compresses and prunes tool input JSON schemas to conserve system prompt token budget.
//!
//! Strips verbose schema annotations, redundant descriptions, and compresses definitions.

use serde_json::{Value as JsonValue, json};

pub const DEFAULT_COMPACT_TOOL_SCHEMA_BYTES: usize = 5_000;
pub const MAX_COMPACT_TOOL_SCHEMA_DEPTH: usize = 3;

const DEFINITION_KEYS: &[&str] = &["$defs", "definitions"];
const COMPOSITION_KEYS: &[&str] = &["anyOf", "oneOf", "allOf"];
const CHILD_SCHEMA_KEYS: &[&str] = &[
    "properties",
    "patternProperties",
    "additionalProperties",
    "items",
    "prefixItems",
    "anyOf",
    "oneOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
];

/// Compresses unusually large tool schemas to fit within `max_bytes` while preserving
/// the top-level argument interface. Runs progressive passes until within budget.
pub fn compact_large_tool_schema(value: &mut JsonValue, max_bytes: usize) {
    let passes: &[fn(&mut JsonValue)] = &[
        strip_nested_descriptions,
        drop_definitions,
        collapse_deep_objects,
        prune_compositions,
    ];

    for pass in passes {
        if schema_byte_len(value) <= max_bytes {
            break;
        }
        pass(value);
    }
}

pub fn schema_byte_len(value: &JsonValue) -> usize {
    serde_json::to_vec(value).map(|v| v.len()).unwrap_or(0)
}

/// Pass 1: Strips nested "description" fields, but preserves top-level property descriptions.
pub fn strip_nested_descriptions(value: &mut JsonValue) {
    if let JsonValue::Object(map) = value
        && let Some(props) = map.get_mut("properties").and_then(|v| v.as_object_mut())
    {
        for prop in props.values_mut() {
            if let JsonValue::Object(prop_map) = prop {
                for (k, v) in prop_map.iter_mut() {
                    if k != "description" {
                        strip_descriptions_recursively(v);
                    }
                }
            }
        }
    }
}

fn strip_descriptions_recursively(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            map.remove("description");
            for v in map.values_mut() {
                strip_descriptions_recursively(v);
            }
        }
        JsonValue::Array(arr) => {
            for v in arr {
                strip_descriptions_recursively(v);
            }
        }
        _ => {}
    }
}

/// Pass 2: Drops `$defs` and `definitions` tables after clearing local `$ref` pointers.
pub fn drop_definitions(value: &mut JsonValue) {
    clear_local_refs(value);
    if let JsonValue::Object(map) = value {
        for key in DEFINITION_KEYS {
            map.remove(*key);
        }
    }
}

fn clear_local_refs(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            if map.contains_key("$ref") {
                *value = json!({});
                return;
            }
            for v in map.values_mut() {
                clear_local_refs(v);
            }
        }
        JsonValue::Array(arr) => {
            for v in arr {
                clear_local_refs(v);
            }
        }
        _ => {}
    }
}

/// Pass 3: Collapses schema objects deeper than level 3 to empty `{}` schemas.
pub fn collapse_deep_objects(value: &mut JsonValue) {
    collapse_objects_at_depth(value, 0);
}

fn collapse_objects_at_depth(value: &mut JsonValue, depth: usize) {
    match value {
        JsonValue::Object(map) => {
            if depth >= MAX_COMPACT_TOOL_SCHEMA_DEPTH && is_complex_object(map) {
                *value = json!({});
                return;
            }
            for key in CHILD_SCHEMA_KEYS {
                if let Some(child) = map.get_mut(*key) {
                    collapse_objects_at_depth(child, depth + 1);
                }
            }
        }
        JsonValue::Array(arr) => {
            for v in arr {
                collapse_objects_at_depth(v, depth);
            }
        }
        _ => {}
    }
}

fn is_complex_object(map: &serde_json::Map<String, JsonValue>) -> bool {
    map.contains_key("properties")
        || map.contains_key("additionalProperties")
        || map.contains_key("items")
        || map.contains_key("$ref")
}

/// Pass 4: Prunes union/composition keywords (`anyOf`, `oneOf`, `allOf`) to `{}`.
pub fn prune_compositions(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            for key in COMPOSITION_KEYS {
                if map.contains_key(*key) {
                    *value = json!({});
                    return;
                }
            }
            for v in map.values_mut() {
                prune_compositions(v);
            }
        }
        JsonValue::Array(arr) => {
            for v in arr {
                prune_compositions(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_nested_descriptions() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "description": "User profile",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "User's full name"
                        }
                    }
                }
            }
        });

        strip_nested_descriptions(&mut schema);
        // Top-level property description preserved, but nested sub-property description removed
        let user_prop = schema["properties"]["user"].as_object().unwrap();
        assert!(user_prop.contains_key("description"));
        let name_prop = schema["properties"]["user"]["properties"]["name"]
            .as_object()
            .unwrap();
        assert!(!name_prop.contains_key("description"));
    }

    #[test]
    fn test_drop_definitions() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "target": {
                    "$ref": "#/$defs/Target"
                }
            },
            "$defs": {
                "Target": {
                    "type": "string"
                }
            }
        });

        drop_definitions(&mut schema);
        assert!(!schema.as_object().unwrap().contains_key("$defs"));
        assert_eq!(schema["properties"]["target"], json!({}));
    }

    #[test]
    fn test_compact_large_tool_schema_budget() {
        let mut huge_schema = json!({
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "object",
                            "properties": {
                                "level3": {
                                    "type": "object",
                                    "properties": {
                                        "level4": {
                                            "type": "string",
                                            "description": "Nested leaf data that is extremely large and verbose and takes a lot of bytes"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let initial_len = schema_byte_len(&huge_schema);
        compact_large_tool_schema(&mut huge_schema, 100);
        let compacted_len = schema_byte_len(&huge_schema);

        assert!(compacted_len < initial_len);
    }
}
