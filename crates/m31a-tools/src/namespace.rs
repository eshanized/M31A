//! Tool namespace management, prefix guidance, and conflict resolution.
//! Manages tool namespacing, prefix stripping, and alias resolution to prevent naming collisions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Error returned when conflicting prefix guidance is configured for the same namespace.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
#[error(
    "Conflicting indirect description prefixes for namespace `{namespace}`: `{first_source}` and `{second_source}`"
)]
pub struct IndirectNamespacePrefixConflict {
    pub namespace: String,
    pub first_source: String,
    pub second_source: String,
}

/// Structured tool name with optional namespace hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ToolName {
    pub namespace: Option<String>,
    pub name: String,
}

impl ToolName {
    pub fn new(namespace: Option<impl Into<String>>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.map(|n| n.into()),
            name: name.into(),
        }
    }

    /// Parses a raw tool identifier into namespace and name.
    /// Double underscore `__` denotes namespace separator (e.g. `mcp__github__create_issue`).
    pub fn parse(raw: &str) -> Self {
        if let Some(idx) = raw.rfind("__") {
            let ns = &raw[..idx];
            let name = &raw[idx + 2..];
            Self {
                namespace: Some(ns.to_string()),
                name: name.to_string(),
            }
        } else {
            Self {
                namespace: None,
                name: raw.to_string(),
            }
        }
    }

    /// Formats the full identifier including namespace if present.
    pub fn full_name(&self) -> String {
        if let Some(ref ns) = self.namespace {
            format!("{}__{}", ns, self.name)
        } else {
            self.name.clone()
        }
    }

    /// Returns the effective namespace or "default".
    pub fn effective_namespace(&self) -> &str {
        self.namespace.as_deref().unwrap_or("default")
    }
}

/// Description for a tool namespace grouping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolNamespaceDescription {
    pub name: String,
    pub description: String,
}

/// Guidance prefixes applied to tool descriptions based on their namespace.
#[derive(Debug, Clone, Default)]
pub struct IndirectNamespacePrefixes {
    /// Maps namespace name -> (prefix text, source name)
    prefixes: BTreeMap<String, (String, String)>,
}

impl IndirectNamespacePrefixes {
    pub fn new() -> Self {
        Self {
            prefixes: BTreeMap::new(),
        }
    }

    /// Registers guidance for a namespace, verifying there is no conflicting guidance.
    pub fn register_guidance(
        &mut self,
        namespace: &str,
        guidance: &str,
        source: &str,
    ) -> Result<(), IndirectNamespacePrefixConflict> {
        let trimmed = guidance.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        if let Some((existing_text, existing_source)) = self.prefixes.get(namespace) {
            if existing_text != trimmed {
                return Err(IndirectNamespacePrefixConflict {
                    namespace: namespace.to_string(),
                    first_source: existing_source.clone(),
                    second_source: source.to_string(),
                });
            }
        } else {
            self.prefixes.insert(
                namespace.to_string(),
                (trimmed.to_string(), source.to_string()),
            );
        }

        Ok(())
    }

    /// Retrieves registered guidance for a namespace.
    pub fn get_guidance(&self, namespace: &str) -> Option<&str> {
        self.prefixes.get(namespace).map(|(text, _)| text.as_str())
    }

    /// Applies guidance to a single tool description by prepending the namespace instructions.
    pub fn apply_to_description(&self, tool_name: &ToolName, description: &mut String) {
        let ns = tool_name.effective_namespace();
        if let Some(guidance) = self.get_guidance(ns) {
            if description.is_empty() {
                *description = guidance.to_string();
            } else if !description.starts_with(guidance) {
                *description = format!("{}\n\n{}", guidance, description);
            }
        }
    }

    /// Returns the count of configured namespace prefixes.
    pub fn len(&self) -> usize {
        self.prefixes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_parsing() {
        let simple = ToolName::parse("exec_command");
        assert_eq!(simple.namespace, None);
        assert_eq!(simple.name, "exec_command");
        assert_eq!(simple.full_name(), "exec_command");
        assert_eq!(simple.effective_namespace(), "default");

        let namespaced = ToolName::parse("mcp__github__create_issue");
        assert_eq!(namespaced.namespace.as_deref(), Some("mcp__github"));
        assert_eq!(namespaced.name, "create_issue");
        assert_eq!(namespaced.full_name(), "mcp__github__create_issue");
        assert_eq!(namespaced.effective_namespace(), "mcp__github");
    }

    #[test]
    fn test_indirect_namespace_guidance_application() {
        let mut prefixes = IndirectNamespacePrefixes::new();
        prefixes
            .register_guidance(
                "mcp__github",
                "Operates on GitHub repository issues and PRs. Always verify issue numbers before updating.",
                "server_config",
            )
            .expect("register guidance");

        let tool_name = ToolName::parse("mcp__github__close_issue");
        let mut desc = "Closes an existing issue.".to_string();
        prefixes.apply_to_description(&tool_name, &mut desc);

        assert!(desc.starts_with("Operates on GitHub repository issues and PRs."));
        assert!(desc.contains("Closes an existing issue."));
    }

    #[test]
    fn test_indirect_namespace_conflict_detection() {
        let mut prefixes = IndirectNamespacePrefixes::new();
        prefixes
            .register_guidance(
                "mcp__db",
                "Access production database in read-only mode.",
                "policy_1",
            )
            .unwrap();

        // Registering identical text from another source is permitted
        assert!(
            prefixes
                .register_guidance(
                    "mcp__db",
                    "Access production database in read-only mode.",
                    "policy_2"
                )
                .is_ok()
        );

        // Registering conflicting text returns an error
        let err = prefixes
            .register_guidance(
                "mcp__db",
                "Access production database in READ-WRITE mode.",
                "policy_3",
            )
            .unwrap_err();

        assert_eq!(err.namespace, "mcp__db");
        assert_eq!(err.first_source, "policy_1");
        assert_eq!(err.second_source, "policy_3");
    }
}
