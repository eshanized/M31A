//! Canonical JSON Schema generator for M31A configuration files.
//!
//! Synthesizes JSON Schema (Draft-07) specifications used by editors for validation and autocompletion.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

/// Generates the canonical JSON Schema (Draft-07) for `config.toml`.
/// Generates the canonical JSON Schema (Draft-07) for `config.toml` validation and autocompletion.
pub fn generate_config_schema() -> Value {
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "M31A Configuration Schema",
        "description": "Schema definition for M31A configuration files (~/.config/m31a/config.toml or .m31a/config.toml)",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "model": {
                "type": "string",
                "description": "Default model identifier from NVIDIA NIM or OpenAI compatible catalog.",
                "default": "nvidia/nemotron-3.5-lightning-30b-a3b",
                "examples": [
                    "nvidia/nemotron-3.5-lightning-30b-a3b",
                    "nvidia/nemotron-3.5-bolt-e4b",
                    "nvidia/llama-3.1-nemotron-70b-instruct",
                    "deepseek-ai/deepseek-v4.1-flash",
                    "mistralai/codestral-22b-instruct-v0.1",
                    "google/gemma-3-12b-it"
                ]
            },
            "temperature": {
                "type": "number",
                "description": "Sampling temperature for model generation (lower is more deterministic).",
                "minimum": 0.0,
                "maximum": 2.0,
                "default": 0.2
            },
            "top_p": {
                "type": "number",
                "description": "Nucleus sampling probability threshold.",
                "minimum": 0.0,
                "maximum": 1.0,
                "default": 0.95
            },
            "max_tokens": {
                "type": "integer",
                "description": "Maximum number of completion tokens generated per model turn.",
                "minimum": 1,
                "maximum": 131072,
                "default": 8192
            },
            "sandbox": {
                "type": "string",
                "description": "Filesystem sandbox isolation policy.",
                "enum": ["read-only", "workspace-write", "danger-full-access"],
                "default": "workspace-write"
            },
            "approval": {
                "type": "string",
                "description": "Tool execution approval policy.",
                "enum": ["on-request", "never", "approve-for-me"],
                "default": "on-request"
            },
            "model_auto_compact_token_limit": {
                "type": "integer",
                "description": "Token budget threshold that triggers automated history summarization and compaction.",
                "minimum": 1000,
                "default": 96000
            },
            "system_prompt": {
                "type": "string",
                "description": "Custom system instructions injected at the top of every agent session."
            },
            "notify": {
                "type": "array",
                "description": "External command line invoked when an agent run finishes or requires human input.",
                "items": { "type": "string" }
            },
            "proxy": {
                "type": "object",
                "description": "Network security proxy and MITM policy settings.",
                "properties": {
                    "socks5_port": { "type": "integer", "minimum": 1024, "maximum": 65535, "default": 10808 },
                    "http_port": { "type": "integer", "minimum": 1024, "maximum": 65535, "default": 10809 },
                    "reject_unencrypted": { "type": "boolean", "default": true },
                    "allowed_domains": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "denied_domains": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            },
            "guardian": {
                "type": "object",
                "description": "Guardian safety reviewer and heuristic circuit breaker configuration.",
                "properties": {
                    "enabled": { "type": "boolean", "default": true },
                    "auto_approve_benign": { "type": "boolean", "default": true },
                    "circuit_breaker_trips": { "type": "integer", "minimum": 1, "default": 3 }
                }
            },
            "mcp_servers": {
                "type": "object",
                "description": "Configured Model Context Protocol (MCP) server backends.",
                "additionalProperties": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Binary or executable name to launch" },
                        "args": { "type": "array", "items": { "type": "string" } },
                        "env": { "type": "object", "additionalProperties": { "type": "string" } }
                    },
                    "required": ["command"]
                }
            }
        }
    })
}

/// Writes the canonical JSON Schema to a file on disk.
pub fn write_config_schema(out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create schema directory at {}", parent.display())
        })?;
    }

    let schema = generate_config_schema();
    let json_bytes = serde_json::to_vec_pretty(&schema)?;
    fs::write(out_path, json_bytes)
        .with_context(|| format!("Failed to write schema to {}", out_path.display()))?;

    Ok(())
}

/// Basic structural validation of a loaded config against the schema.
pub fn validate_config_structure(config: &Value) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some(obj) = config.as_object() {
        if let Some(sandbox) = obj.get("sandbox")
            && let Some(s) = sandbox.as_str()
            && !["read-only", "workspace-write", "danger-full-access"].contains(&s)
        {
            warnings.push(format!("Invalid sandbox policy '{}'", s));
        }
        if let Some(approval) = obj.get("approval")
            && let Some(a) = approval.as_str()
            && !["on-request", "never", "approve-for-me"].contains(&a)
        {
            warnings.push(format!("Invalid approval policy '{}'", a));
        }
        if let Some(temp) = obj.get("temperature")
            && let Some(t) = temp.as_f64()
            && (t < 0.0 || t > 2.0)
        {
            warnings.push(format!(
                "Temperature {} outside recommended range [0.0, 2.0]",
                t
            ));
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_config_schema() {
        let schema = generate_config_schema();
        assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
        assert!(
            schema["properties"]["model"]["examples"]
                .as_array()
                .unwrap()
                .len()
                >= 5
        );
        assert_eq!(
            schema["properties"]["sandbox"]["default"],
            "workspace-write"
        );

        let temp_dir = tempfile::tempdir().unwrap();
        let schema_path = temp_dir.path().join("config.schema.json");
        write_config_schema(&schema_path).unwrap();
        assert!(schema_path.exists());

        // Validate structure
        let valid_config = json!({
            "model": "nvidia/nemotron-3.5-lightning-30b-a3b",
            "temperature": 0.5,
            "sandbox": "workspace-write",
            "approval": "approve-for-me"
        });
        assert!(validate_config_structure(&valid_config).is_empty());

        let invalid_config = json!({
            "sandbox": "ultra-unrestricted-mode",
            "temperature": 5.0
        });
        let warnings = validate_config_structure(&invalid_config);
        assert_eq!(warnings.len(), 2);
    }
}
