//! Declarative policy document and rule AST definitions (POL-01, POL-02, D-01).

use serde::{Deserialize, Serialize};

use crate::kernel::seams::policy::PolicyDecision;
use crate::state::intake::AutonomyMode;
use crate::state_machine::agent::AgentRole;

/// Current declarative policy schema version.
pub const CURRENT_POLICY_SCHEMA_VERSION: &str = "1.0";

/// Root declarative policy document parsed from TOML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyDocument {
    pub version: String,
    pub name: Option<String>,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl PolicyDocument {
    /// Create a new policy document with schema version 1.0.
    pub fn new(name: Option<String>, rules: Vec<PolicyRule>) -> Self {
        Self {
            version: CURRENT_POLICY_SCHEMA_VERSION.to_string(),
            name,
            rules,
        }
    }
}

/// Strongly typed declarative policy rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyRule {
    pub id: String,
    pub description: Option<String>,
    pub decision: PolicyDecision,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
    #[serde(default)]
    pub roles: Vec<AgentRole>,
    #[serde(default)]
    pub modes: Vec<AutonomyMode>,
}

impl PolicyRule {
    /// Create a minimal policy rule with an ID and a decision.
    pub fn new(id: impl Into<String>, decision: PolicyDecision) -> Self {
        Self {
            id: id.into(),
            description: None,
            decision,
            tools: Vec::new(),
            paths: Vec::new(),
            args: None,
            roles: Vec::new(),
            modes: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tools = tools.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_paths(mut self, paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.paths = paths.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.args = Some(args);
        self
    }

    pub fn with_roles(mut self, roles: impl IntoIterator<Item = AgentRole>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    pub fn with_modes(mut self, modes: impl IntoIterator<Item = AutonomyMode>) -> Self {
        self.modes = modes.into_iter().collect();
        self
    }
}
