//! Semantic JSON to TOML converter.
//!
//! Converts arbitrary `serde_json::Value` structures into idiomatic `toml::Value` representations,
//! enabling configuration format migration, prompt serialization, and schema transcoding.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

/// Convert a `serde_json::Value` into an equivalent `toml::Value`.
pub fn json_to_toml(v: JsonValue) -> TomlValue {
    match v {
        JsonValue::Null => TomlValue::String(String::new()),
        JsonValue::Bool(b) => TomlValue::Boolean(b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                TomlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                TomlValue::Float(f)
            } else {
                TomlValue::String(n.to_string())
            }
        }
        JsonValue::String(s) => TomlValue::String(s),
        JsonValue::Array(arr) => TomlValue::Array(arr.into_iter().map(json_to_toml).collect()),
        JsonValue::Object(map) => {
            let tbl = map
                .into_iter()
                .map(|(k, v)| (k, json_to_toml(v)))
                .collect::<toml::value::Table>();
            TomlValue::Table(tbl)
        }
    }
}

/// Parse a JSON string and transcode it to a canonical TOML string.
pub fn json_str_to_toml_str(json_str: &str) -> Result<String> {
    let json_val: JsonValue =
        serde_json::from_str(json_str).context("Failed to parse input as valid JSON")?;
    let toml_val = json_to_toml(json_val);
    toml::to_string_pretty(&toml_val).context("Failed to serialize toml::Value to string")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_number_conversion() {
        let v_int = json!(42);
        assert_eq!(json_to_toml(v_int), TomlValue::Integer(42));

        let v_float = json!(3.25);
        assert_eq!(json_to_toml(v_float), TomlValue::Float(3.25));
    }

    #[test]
    fn test_json_nested_structure() {
        let val = json!({
            "project": "m31a",
            "active": true,
            "limits": {
                "max_tokens": 8192,
                "temperature": 0.2
            },
            "tags": ["agentic", "sovereign", "rust"]
        });

        let toml_val = json_to_toml(val);
        match toml_val {
            TomlValue::Table(tbl) => {
                assert_eq!(
                    tbl.get("project"),
                    Some(&TomlValue::String("m31a".to_string()))
                );
                assert_eq!(tbl.get("active"), Some(&TomlValue::Boolean(true)));
                let limits = tbl.get("limits").unwrap().as_table().unwrap();
                assert_eq!(limits.get("max_tokens"), Some(&TomlValue::Integer(8192)));
            }
            _ => panic!("Expected TomlValue::Table"),
        }
    }

    #[test]
    fn test_json_str_to_toml_str() {
        let json_input = r#"{"name": "m31a", "version": "0.1.0"}"#;
        let toml_out = json_str_to_toml_str(json_input).unwrap();
        assert!(toml_out.contains(r#"name = "m31a""#));
        assert!(toml_out.contains(r#"version = "0.1.0""#));
    }
}
