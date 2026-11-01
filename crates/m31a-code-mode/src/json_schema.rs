//! Converts JSON Schema definitions into strongly-typed TypeScript type declarations.
//!
//! Synthesizes TypeScript interfaces from tool input schemas to provide IntelliSense and compile-time
//! validation inside Code Mode scripts.

use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

pub const DEFAULT_INPUT_SCHEMA_MAX_BYTES: usize = 16_000;
const MAX_LOCAL_REF_EXPANSIONS_PER_PATH: usize = 2;
const MAX_TOTAL_LOCAL_REF_EXPANSIONS: usize = 32;
const RENDER_WORK_MULTIPLIER: usize = 4;

/// Renders a JSON Schema into an idiomatic TypeScript type definition string.
pub fn render_json_schema_to_typescript(schema: &JsonValue) -> String {
    render_json_schema_to_typescript_with_budget(schema, DEFAULT_INPUT_SCHEMA_MAX_BYTES)
}

/// Renders a JSON Schema into a TypeScript type definition string with an explicit byte budget.
pub fn render_json_schema_to_typescript_with_budget(
    schema: &JsonValue,
    max_bytes: usize,
) -> String {
    let mut renderer = JsonSchemaTypeRenderer::new(schema, max_bytes);
    let rendered = renderer.render(schema);
    if rendered.len() > max_bytes {
        "unknown".to_string()
    } else {
        rendered
    }
}

pub const MCP_TYPESCRIPT_PREAMBLE: &str = r#"type Role = "user" | "assistant";
type MetaObject = Record<string, unknown>;
type Annotations = {
  audience?: Role[];
  priority?: number;
  lastModified?: string;
};
type Icon = {
  src: string;
  mimeType?: string;
  sizes?: string[];
  theme?: "light" | "dark";
};
type TextResourceContents = {
  uri: string;
  mimeType?: string;
  _meta?: MetaObject;
  text: string;
};
type BlobResourceContents = {
  uri: string;
  mimeType?: string;
  _meta?: MetaObject;
  blob: string;
};
type TextContent = {
  type: "text";
  text: string;
  annotations?: Annotations;
  _meta?: MetaObject;
};
type ImageContent = {
  type: "image";
  data: string;
  mimeType: string;
  annotations?: Annotations;
  _meta?: MetaObject;
};
type AudioContent = {
  type: "audio";
  data: string;
  mimeType: string;
  annotations?: Annotations;
  _meta?: MetaObject;
};
type ResourceLink = {
  icons?: Icon[];
  name: string;
  title?: string;
  uri: string;
  description?: string;
  mimeType?: string;
  annotations?: Annotations;
  size?: number;
  _meta?: MetaObject;
  type: "resource_link";
};
type EmbeddedResource = {
  type: "resource";
  resource: TextResourceContents | BlobResourceContents;
  annotations?: Annotations;
  _meta?: MetaObject;
};
type ContentBlock =
  | TextContent
  | ImageContent
  | AudioContent
  | ResourceLink
  | EmbeddedResource;
type CallToolResult<TStructured = { [key: string]: unknown }> = {
  _meta?: MetaObject;
  content: ContentBlock[];
  isError?: boolean;
  structuredContent?: TStructured;
  [key: string]: unknown;
};"#;

struct JsonSchemaTypeRenderer<'a> {
    root: &'a JsonValue,
    nested_schema_resource_depth: usize,
    active_local_ref_expansions: BTreeMap<String, usize>,
    remaining_local_ref_expansions: usize,
    remaining_render_work_bytes: usize,
    render_work_budget_exhausted: bool,
}

impl<'a> JsonSchemaTypeRenderer<'a> {
    fn new(root: &'a JsonValue, max_bytes: usize) -> Self {
        Self {
            root,
            nested_schema_resource_depth: 0,
            active_local_ref_expansions: BTreeMap::new(),
            remaining_local_ref_expansions: MAX_TOTAL_LOCAL_REF_EXPANSIONS,
            remaining_render_work_bytes: max_bytes.saturating_mul(RENDER_WORK_MULTIPLIER),
            render_work_budget_exhausted: false,
        }
    }

    fn render(&mut self, schema: &JsonValue) -> String {
        if self.render_work_budget_exhausted {
            return "unknown".to_string();
        }

        let enters_nested = !std::ptr::eq(schema, self.root)
            && schema.as_object().is_some_and(|m| m.contains_key("$id"));
        if enters_nested {
            self.nested_schema_resource_depth += 1;
        }

        let rendered = match schema {
            JsonValue::Bool(true) => "unknown".to_string(),
            JsonValue::Bool(false) => "never".to_string(),
            JsonValue::Object(map) => self.render_map(map),
            _ => "unknown".to_string(),
        };

        if enters_nested {
            self.nested_schema_resource_depth -= 1;
        }

        self.finish_render(rendered)
    }

    fn render_map(&mut self, map: &serde_json::Map<String, JsonValue>) -> String {
        if self.render_work_budget_exhausted {
            return "unknown".to_string();
        }

        if map.contains_key("$ref") {
            return self.render_ref(map);
        }

        if let Some(value) = map.get("const") {
            return self.render_literal(value);
        }

        if let Some(values) = map.get("enum").and_then(JsonValue::as_array) {
            let mut rendered = Vec::new();
            for value in values {
                let literal = self.render_literal(value);
                if self.render_work_budget_exhausted || !self.consume_render_work(literal.len()) {
                    return "unknown".to_string();
                }
                rendered.push(literal);
            }
            if !rendered.is_empty() {
                return rendered.join(" | ");
            }
        }

        for key in ["anyOf", "oneOf"] {
            if let Some(variants) = map.get(key).and_then(JsonValue::as_array) {
                let mut rendered = Vec::new();
                for variant in variants {
                    if self.render_work_budget_exhausted {
                        return "unknown".to_string();
                    }
                    rendered.push(self.render(variant));
                }
                if !rendered.is_empty() {
                    return rendered.join(" | ");
                }
            }
        }

        if let Some(variants) = map.get("allOf").and_then(JsonValue::as_array) {
            let mut rendered = Vec::new();
            for variant in variants {
                if self.render_work_budget_exhausted {
                    return "unknown".to_string();
                }
                let r = self.render(variant);
                if r.contains(" | ") {
                    rendered.push(format!("({r})"));
                } else {
                    rendered.push(r);
                }
            }
            if !rendered.is_empty() {
                return rendered.join(" & ");
            }
        }

        if let Some(schema_type) = map.get("type") {
            if let Some(types) = schema_type.as_array() {
                let mut rendered = Vec::new();
                for t in types.iter().filter_map(JsonValue::as_str) {
                    if self.render_work_budget_exhausted {
                        return "unknown".to_string();
                    }
                    rendered.push(self.render_type_keyword(map, t));
                }
                if !rendered.is_empty() {
                    return rendered.join(" | ");
                }
            }

            if let Some(t) = schema_type.as_str() {
                return self.render_type_keyword(map, t);
            }
        }

        if map.contains_key("properties")
            || map.contains_key("additionalProperties")
            || map.contains_key("required")
        {
            return self.render_object(map);
        }

        if map.contains_key("items") || map.contains_key("prefixItems") {
            return self.render_array(map);
        }

        "unknown".to_string()
    }

    fn render_ref(&mut self, map: &serde_json::Map<String, JsonValue>) -> String {
        if self.nested_schema_resource_depth > 0 {
            return "unknown".to_string();
        }

        let pointer = match map.get("$ref").and_then(JsonValue::as_str) {
            Some(ref_str) if ref_str.starts_with('#') => ref_str.trim_start_matches('#'),
            _ => return "unknown".to_string(),
        };

        let active = self
            .active_local_ref_expansions
            .get(pointer)
            .copied()
            .unwrap_or(0);
        if active >= MAX_LOCAL_REF_EXPANSIONS_PER_PATH || self.remaining_local_ref_expansions == 0 {
            return "unknown".to_string();
        }

        let target = if pointer.is_empty() {
            Some(self.root)
        } else {
            self.root.pointer(pointer)
        };

        let Some(target) = target else {
            return "unknown".to_string();
        };

        self.remaining_local_ref_expansions -= 1;
        self.active_local_ref_expansions
            .insert(pointer.to_string(), active + 1);

        let rendered = self.render(target);
        if active == 0 {
            self.active_local_ref_expansions.remove(pointer);
        } else {
            self.active_local_ref_expansions
                .insert(pointer.to_string(), active);
        }

        rendered
    }

    fn render_type_keyword(
        &mut self,
        map: &serde_json::Map<String, JsonValue>,
        schema_type: &str,
    ) -> String {
        match schema_type {
            "string" => "string".to_string(),
            "number" | "integer" => "number".to_string(),
            "boolean" => "boolean".to_string(),
            "null" => "null".to_string(),
            "array" => self.render_array(map),
            "object" => self.render_object(map),
            _ => "unknown".to_string(),
        }
    }

    fn render_array(&mut self, map: &serde_json::Map<String, JsonValue>) -> String {
        if let Some(items) = map.get("items") {
            let item_type = self.render(items);
            if self.render_work_budget_exhausted {
                return "unknown".to_string();
            }
            return format!("Array<{item_type}>");
        }

        if let Some(items) = map.get("prefixItems").and_then(JsonValue::as_array) {
            let mut item_types = Vec::new();
            for item in items {
                if self.render_work_budget_exhausted {
                    return "unknown".to_string();
                }
                item_types.push(self.render(item));
            }
            if !item_types.is_empty() {
                return format!("[{}]", item_types.join(", "));
            }
        }

        "unknown[]".to_string()
    }

    fn render_object(&mut self, map: &serde_json::Map<String, JsonValue>) -> String {
        let required_vec = map
            .get("required")
            .and_then(JsonValue::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let empty_properties = serde_json::Map::new();
        let properties = map
            .get("properties")
            .and_then(JsonValue::as_object)
            .unwrap_or(&empty_properties);

        let mut lines = Vec::new();
        for (name, value) in properties {
            if self.render_work_budget_exhausted {
                return "unknown".to_string();
            }
            let is_req = required_vec.contains(&name.as_str());
            let opt = if is_req { "" } else { "?" };
            let prop_name = render_property_name(name);
            let prop_type = self.render(value);
            lines.push(format!("  {prop_name}{opt}: {prop_type};"));
        }

        if let Some(additional) = map.get("additionalProperties") {
            let add_type = match additional {
                JsonValue::Bool(true) => Some("unknown".to_string()),
                JsonValue::Bool(false) => None,
                val => Some(self.render(val)),
            };
            if let Some(t) = add_type {
                lines.push(format!("  [key: string]: {t};"));
            }
        } else if properties.is_empty() {
            lines.push("  [key: string]: unknown;".to_string());
        }

        if lines.is_empty() {
            "{}".to_string()
        } else {
            format!("{{\n{}\n}}", lines.join("\n"))
        }
    }

    fn render_literal(&mut self, value: &JsonValue) -> String {
        match value {
            JsonValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            JsonValue::Number(n) => n.to_string(),
            JsonValue::Bool(b) => b.to_string(),
            JsonValue::Null => "null".to_string(),
            _ => "unknown".to_string(),
        }
    }

    fn consume_render_work(&mut self, amount: usize) -> bool {
        if amount > self.remaining_render_work_bytes {
            self.render_work_budget_exhausted = true;
            false
        } else {
            self.remaining_render_work_bytes -= amount;
            true
        }
    }

    fn finish_render(&mut self, rendered: String) -> String {
        if self.render_work_budget_exhausted {
            "unknown".to_string()
        } else {
            rendered
        }
    }
}

fn render_property_name(name: &str) -> String {
    let is_valid_ident = !name.is_empty()
        && (name.chars().next().unwrap().is_ascii_alphabetic()
            || name.starts_with('_')
            || name.starts_with('$'))
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');

    if is_valid_ident {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_primitive_types() {
        assert_eq!(
            render_json_schema_to_typescript(&json!({"type": "string"})),
            "string"
        );
        assert_eq!(
            render_json_schema_to_typescript(&json!({"type": "number"})),
            "number"
        );
        assert_eq!(
            render_json_schema_to_typescript(&json!({"type": "boolean"})),
            "boolean"
        );
        assert_eq!(
            render_json_schema_to_typescript(&json!({"type": "null"})),
            "null"
        );
    }

    #[test]
    fn test_render_object_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" },
                "tags": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["name"]
        });

        let ts = render_json_schema_to_typescript(&schema);
        assert!(ts.contains("name: string;"));
        assert!(ts.contains("age?: number;"));
        assert!(ts.contains("tags?: Array<string>;"));
    }

    #[test]
    fn test_render_enum_and_unions() {
        let schema = json!({
            "type": "string",
            "enum": ["fast", "balanced", "thorough"]
        });
        assert_eq!(
            render_json_schema_to_typescript(&schema),
            "\"fast\" | \"balanced\" | \"thorough\""
        );

        let union_schema = json!({
            "anyOf": [
                { "type": "string" },
                { "type": "number" }
            ]
        });
        assert_eq!(
            render_json_schema_to_typescript(&union_schema),
            "string | number"
        );
    }
}
