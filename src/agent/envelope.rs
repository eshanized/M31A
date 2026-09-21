//! Role capability envelopes and task-assignment capability intersection logic (AGT-04, D-02).
//!
//! Enforces compile-time and runtime boundaries on agent actions, guaranteeing
//! that task requirements cannot exceed role privilege envelopes.

use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Immutable capability envelope defining the maximum privilege boundary for an agent role (D-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    pub allowed_capabilities: BTreeSet<String>,
    pub allow_file_write: bool,
    pub allow_shell_execution: bool,
    pub allow_network_access: bool,
}

impl CapabilityEnvelope {
    /// Create a strictly read-only capability envelope.
    pub fn read_only(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed_capabilities: allowed.into_iter().map(Into::into).collect(),
            allow_file_write: false,
            allow_shell_execution: false,
            allow_network_access: false,
        }
    }

    /// Create a workspace-mutating capability envelope with filesystem write and shell execution.
    pub fn workspace_write(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed_capabilities: allowed.into_iter().map(Into::into).collect(),
            allow_file_write: true,
            allow_shell_execution: true,
            allow_network_access: false,
        }
    }

    /// Alias for full workspace capability envelope.
    pub fn full_workspace(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::workspace_write(allowed)
    }

    /// Create a capability envelope with network read access.
    pub fn network_read(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed_capabilities: allowed.into_iter().map(Into::into).collect(),
            allow_file_write: false,
            allow_shell_execution: false,
            allow_network_access: true,
        }
    }
}

/// Errors raised when task capability requirements exceed the role's capability envelope (AGT-04).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CapabilityIntersectionError {
    #[error("task requires capability '{0}' forbidden by role envelope")]
    ForbiddenCapability(String),
    #[error("task requires write access, but role envelope is strictly read-only")]
    WriteAccessForbidden,
    #[error("task requires shell execution, but role envelope forbids shell execution")]
    ShellExecutionForbidden,
    #[error("task requires network access, but role envelope forbids network access")]
    NetworkAccessForbidden,
}

/// Computes the eligible capability scope by intersecting task requirements with role envelope (D-02, AGT-04).
///
/// Fails closed immediately if any requested capability is unpermitted, if write access is
/// requested on a read-only role, or if shell/network access is requested without envelope authorization.
pub fn calculate_eligible_capabilities(
    task_capabilities: &[CapabilityRequirement],
    role_envelope: &CapabilityEnvelope,
) -> Result<Vec<CapabilityRequirement>, CapabilityIntersectionError> {
    for req in task_capabilities {
        // Skip role routing tags (e.g. "role:implementer") — they designate target
        // role dispatch routing (AGT-04), not tool capabilities.
        if req.id.starts_with("role:") {
            continue;
        }

        // 1. Explicit capability identifier membership
        if !role_envelope.allowed_capabilities.contains(&req.id) {
            return Err(CapabilityIntersectionError::ForbiddenCapability(
                req.id.clone(),
            ));
        }
        // 2. Mutating write access ceiling
        if (req.mode == CapabilityAccessMode::Write || req.mode == CapabilityAccessMode::ReadWrite)
            && !role_envelope.allow_file_write
        {
            return Err(CapabilityIntersectionError::WriteAccessForbidden);
        }
        // 3. Shell execution restriction
        if req.id.starts_with("shell") && !role_envelope.allow_shell_execution {
            return Err(CapabilityIntersectionError::ShellExecutionForbidden);
        }
        // 4. Network access restriction
        if req.id.starts_with("network") && !role_envelope.allow_network_access {
            return Err(CapabilityIntersectionError::NetworkAccessForbidden);
        }
    }
    Ok(task_capabilities.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_envelope_rejects_write() {
        let envelope = CapabilityEnvelope::read_only(["fs.read", "repo.inspect"]);
        let reqs = vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Write),
        ];
        let err = calculate_eligible_capabilities(&reqs, &envelope).unwrap_err();
        assert_eq!(err, CapabilityIntersectionError::WriteAccessForbidden);
    }

    #[test]
    fn test_read_only_envelope_rejects_shell() {
        let envelope = CapabilityEnvelope::read_only(["shell.exec", "fs.read"]);
        let reqs = vec![CapabilityRequirement::new(
            "shell.exec",
            CapabilityAccessMode::Read,
        )];
        let err = calculate_eligible_capabilities(&reqs, &envelope).unwrap_err();
        assert_eq!(err, CapabilityIntersectionError::ShellExecutionForbidden);
    }

    #[test]
    fn test_workspace_write_envelope_allows_write_and_shell() {
        let envelope = CapabilityEnvelope::workspace_write(["fs.write", "fs.read", "shell.exec"]);
        let reqs = vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("fs.write", CapabilityAccessMode::Write),
            CapabilityRequirement::new("shell.exec", CapabilityAccessMode::Read),
        ];
        let eligible = calculate_eligible_capabilities(&reqs, &envelope).unwrap();
        assert_eq!(eligible.len(), 3);
    }

    #[test]
    fn test_out_of_envelope_capability_fails_closed() {
        let envelope = CapabilityEnvelope::read_only(["fs.read"]);
        let reqs = vec![CapabilityRequirement::new(
            "admin.root",
            CapabilityAccessMode::Read,
        )];
        let err = calculate_eligible_capabilities(&reqs, &envelope).unwrap_err();
        assert_eq!(
            err,
            CapabilityIntersectionError::ForbiddenCapability("admin.root".to_string())
        );
    }

    #[test]
    fn test_empty_task_requirements_succeed() {
        let envelope = CapabilityEnvelope::read_only(["fs.read"]);
        let eligible = calculate_eligible_capabilities(&[], &envelope).unwrap();
        assert!(eligible.is_empty());
    }

    #[test]
    fn test_network_read_envelope() {
        let envelope = CapabilityEnvelope::network_read(["network.http", "fs.read"]);
        let reqs = vec![CapabilityRequirement::new(
            "network.http",
            CapabilityAccessMode::Read,
        )];
        let eligible = calculate_eligible_capabilities(&reqs, &envelope).unwrap();
        assert_eq!(eligible.len(), 1);

        let write_reqs = vec![CapabilityRequirement::new(
            "fs.read",
            CapabilityAccessMode::Write,
        )];
        assert_eq!(
            calculate_eligible_capabilities(&write_reqs, &envelope).unwrap_err(),
            CapabilityIntersectionError::WriteAccessForbidden
        );
    }
}
