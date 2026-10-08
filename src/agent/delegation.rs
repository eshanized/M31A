//! Typed multi-agent delegation contracts and runtime delegation protocols (Issue 13).
//!
//! Enforces bounded recursion depth, typed contract specifications, restricted capability envelopes,
//! failure isolation, and parent-child audit trails.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{AgentId, MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

/// Canonical role identifiers eligible for delegation.
pub const CANONICAL_DELEGATION_ROLES: &[&str] = &[
    "architect",
    "tech_lead",
    "planner",
    "implementer",
    "reviewer",
    "tester",
    "researcher",
    "security_auditor",
];

/// Maximum delegation recursion depth.
pub const MAX_DELEGATION_DEPTH_DEFAULT: usize = 3;

/// Errors arising during delegation validation or execution.
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum DelegationError {
    #[error(
        "Delegation rejected: maximum delegation depth of {max_depth} exceeded (current depth {current_depth})"
    )]
    DepthExceeded {
        current_depth: usize,
        max_depth: usize,
    },
    #[error("Delegation rejected: target role '{0}' is invalid or unknown")]
    InvalidRole(String),
    #[error("Delegation rejected: self-delegation to the same role '{0}' is disallowed")]
    SelfDelegationDisallowed(String),
    #[error("Delegation execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Delegation timed out after {0} seconds")]
    Timeout(u64),
}

/// Explicit typed contract for delegating a subtask to a specialized role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DelegationContract {
    pub parent_agent_id: Option<AgentId>,
    pub parent_role: String,
    pub target_role: String,
    pub subtask_title: String,
    pub input_spec: String,
    pub depth: usize,
    pub max_depth: usize,
    pub timeout_secs: Option<u64>,
}

impl DelegationContract {
    /// Construct a new delegation contract with default depth ceiling.
    pub fn new(
        parent_role: impl Into<String>,
        target_role: impl Into<String>,
        subtask_title: impl Into<String>,
        input_spec: impl Into<String>,
        depth: usize,
    ) -> Self {
        Self {
            parent_agent_id: None,
            parent_role: parent_role.into(),
            target_role: target_role.into(),
            subtask_title: subtask_title.into(),
            input_spec: input_spec.into(),
            depth,
            max_depth: MAX_DELEGATION_DEPTH_DEFAULT,
            timeout_secs: Some(300),
        }
    }

    /// Set custom max depth.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set custom timeout.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set parent agent ID.
    pub fn with_parent_agent_id(mut self, agent_id: AgentId) -> Self {
        self.parent_agent_id = Some(agent_id);
        self
    }

    /// Validate contract against safety rules (depth limit, non-empty, role well-formedness).
    pub fn validate(&self) -> Result<(), DelegationError> {
        if self.depth >= self.max_depth {
            return Err(DelegationError::DepthExceeded {
                current_depth: self.depth,
                max_depth: self.max_depth,
            });
        }

        let trimmed_target = self.target_role.trim().to_lowercase();
        if trimmed_target.is_empty() {
            return Err(DelegationError::InvalidRole("empty role".to_string()));
        }

        let target_role_id = AgentRole::new(&trimmed_target);
        if !target_role_id.is_well_formed() {
            return Err(DelegationError::InvalidRole(trimmed_target));
        }

        let trimmed_parent = self.parent_role.trim().to_lowercase();
        if !trimmed_parent.is_empty() && trimmed_parent == trimmed_target {
            return Err(DelegationError::SelfDelegationDisallowed(trimmed_target));
        }

        Ok(())
    }
}

/// Structured outcome returned by a delegated subagent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DelegationOutcome {
    pub success: bool,
    pub target_role: String,
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<String>,
    pub duration_ms: u64,
}

/// Audit trail record tracking multi-agent delegation hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationHierarchyRecord {
    pub id: String,
    pub mission_id: Option<MissionId>,
    pub parent_task_id: Option<TaskId>,
    pub child_task_id: Option<TaskId>,
    pub parent_role: String,
    pub target_role: String,
    pub depth: usize,
    pub subtask_title: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_validation_success() {
        let contract = DelegationContract::new(
            "tech_lead",
            "implementer",
            "Implement feature X",
            "Follow architecture spec in docs/",
            0,
        );
        assert!(contract.validate().is_ok());
    }

    #[test]
    fn test_delegation_depth_limit() {
        let contract = DelegationContract::new("architect", "implementer", "Subtask", "spec", 3)
            .with_max_depth(3);
        assert!(matches!(
            contract.validate(),
            Err(DelegationError::DepthExceeded { .. })
        ));
    }

    #[test]
    fn test_self_delegation_rejected() {
        let contract = DelegationContract::new("implementer", "implementer", "Subtask", "spec", 0);
        assert!(matches!(
            contract.validate(),
            Err(DelegationError::SelfDelegationDisallowed(_))
        ));
    }
}
