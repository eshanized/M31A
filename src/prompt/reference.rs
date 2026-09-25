//! Typed reference to a versioned prompt contract.

use crate::prompt::error::PromptError;
use serde::{Deserialize, Serialize};

/// Typed reference to a versioned prompt contract (e.g. `genesis.discovery.v1` or `discovery` + version 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptReference {
    /// Logical identifier of the prompt contract (e.g. "genesis.discovery").
    pub id: String,
    /// Exact version number of the contract.
    pub version: u32,
}

impl PromptReference {
    /// Create a new exact prompt reference.
    pub fn new(id: impl Into<String>, version: u32) -> Self {
        Self {
            id: id.into(),
            version,
        }
    }

    /// Return the canonical default prompt reference for a given role id.
    ///
    /// Resolves through the role registry: registered roles use their
    /// declared prompt contract. Unregistered ids follow the `agent.{id}` v1
    /// naming convention without granting existence — prompt existence is
    /// enforced by the catalog and role existence by the registry at
    /// validation boundaries (planning, dispatch, workflow compilation).
    pub fn for_role(role: crate::state_machine::agent::AgentRole) -> Self {
        let registry = crate::agent::registry::RoleRegistry::global();
        let guard = registry.read().expect("role registry lock poisoned");
        guard.prompt_reference_for(&role)
    }

    /// Parse a prompt reference string.
    ///
    /// Accepts formats:
    /// - `name.v1` -> `id: "name", version: 1`
    /// - `genesis.discovery.v2` -> `id: "genesis.discovery", version: 2`
    /// - `discovery:v1` -> `id: "discovery", version: 1`
    /// - `discovery:1` -> `id: "discovery", version: 1`
    /// - `discovery@1` -> `id: "discovery", version: 1`
    pub fn parse(s: &str) -> Result<Self, PromptError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(PromptError::PromptInvalid {
                id: "<empty>".to_string(),
                version: 0,
                reason: "prompt reference string cannot be empty".to_string(),
            });
        }

        // Try colon separator: "name:v1" or "name:1"
        if let Some((id_part, ver_part)) = trimmed.split_once(':') {
            let ver_clean = ver_part.trim_start_matches('v').trim_start_matches('V');
            if let Ok(ver) = ver_clean.parse::<u32>()
                && ver > 0
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // Try @ separator: "name@1" or "name@v1"
        if let Some((id_part, ver_part)) = trimmed.split_once('@') {
            let ver_clean = ver_part.trim_start_matches('v').trim_start_matches('V');
            if let Ok(ver) = ver_clean.parse::<u32>()
                && ver > 0
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // Try dot separator ending in .v<N>: "genesis.discovery.v1"
        if let Some(idx) = trimmed.rfind(".v") {
            let (id_part, ver_part) = trimmed.split_at(idx);
            let ver_digits = &ver_part[2..];
            if let Ok(ver) = ver_digits.parse::<u32>()
                && ver > 0
                && !id_part.trim().is_empty()
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // If no version suffix is present, fail explicitly: exact versioning required
        Err(PromptError::PromptInvalid {
            id: trimmed.to_string(),
            version: 0,
            reason: format!(
                "prompt reference '{}' must include an exact version (e.g. '{}.v1' or '{}:1')",
                trimmed, trimmed, trimmed
            ),
        })
    }
}

impl std::fmt::Display for PromptReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.v{}", self.id, self.version)
    }
}
