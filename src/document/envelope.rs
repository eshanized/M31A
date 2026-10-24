//! Canonical document envelope and supported kind taxonomy.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::document::error::DocumentError;

/// Maximum format schema version supported by this runtime.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Supported canonical document kinds in M31A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// Agent and runtime prompt contracts.
    Prompt,
    /// System, user, or workspace configuration.
    Config,
    /// User settings configuration layer.
    Settings,
    /// Execution profiles with capability & policy overrides.
    Profile,
    /// Declarative policy documents.
    Policy,
    /// Procedural skill definitions.
    Skill,
    /// Verification project build/test adapters.
    Adapter,
    /// User-defined slash commands.
    Command,
    /// Declarative native workflow manifests.
    Workflow,
}

impl DocumentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentKind::Prompt => "prompt",
            DocumentKind::Config => "config",
            DocumentKind::Settings => "settings",
            DocumentKind::Profile => "profile",
            DocumentKind::Policy => "policy",
            DocumentKind::Skill => "skill",
            DocumentKind::Adapter => "adapter",
            DocumentKind::Command => "command",
            DocumentKind::Workflow => "workflow",
        }
    }
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for DocumentKind {
    type Err = DocumentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "prompt" => Ok(DocumentKind::Prompt),
            "config" => Ok(DocumentKind::Config),
            "settings" => Ok(DocumentKind::Settings),
            "profile" => Ok(DocumentKind::Profile),
            "policy" => Ok(DocumentKind::Policy),
            "skill" => Ok(DocumentKind::Skill),
            "adapter" => Ok(DocumentKind::Adapter),
            "command" => Ok(DocumentKind::Command),
            "workflow" => Ok(DocumentKind::Workflow),
            other => Err(DocumentError::UnknownKind(other.to_string())),
        }
    }
}

/// Generic typed document envelope wrapping front-matter metadata and domain data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentEnvelope<T> {
    pub kind: DocumentKind,
    pub schema_version: u32,
    pub data: T,
}

/// A parsed complete Markdown document with strongly typed metadata and preserved body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument<T> {
    pub kind: DocumentKind,
    pub schema_version: u32,
    pub data: T,
    pub body: String,
}

impl<T> MarkdownDocument<T> {
    pub fn new(kind: DocumentKind, schema_version: u32, data: T, body: impl Into<String>) -> Self {
        Self {
            kind,
            schema_version,
            data,
            body: body.into(),
        }
    }

    pub fn envelope(&self) -> DocumentEnvelope<&T> {
        DocumentEnvelope {
            kind: self.kind,
            schema_version: self.schema_version,
            data: &self.data,
        }
    }
}
