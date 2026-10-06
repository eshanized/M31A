//! Strongly typed capability permissions and resource scoping (CTL-03, D-04).

use crate::capability::error::CapabilityError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Capability operations that an action can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityOperation {
    /// Read-only inspection of state or content.
    Read,
    /// Mutating write or creation of state or content.
    Write,
    /// Execution of processes, scripts, or external tools.
    Execute,
    /// Administrative or privileged lifecycle operations.
    Admin,
}

pub use crate::tools::risk::RiskClass;

/// Strongly typed permissions and resource constraints governing capability access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPermissions {
    /// Set of permitted operations (Read, Write, Execute, Admin).
    pub allowed_operations: HashSet<CapabilityOperation>,
    /// Optional workspace path root bounding filesystem operations.
    pub workspace_path_scope: Option<PathBuf>,
    /// Allowed external network domains (empty means no network permitted).
    pub network_domains: Vec<String>,
    /// Maximum allowed risk level.
    pub max_risk: RiskClass,
}

impl Default for CapabilityPermissions {
    fn default() -> Self {
        Self {
            allowed_operations: HashSet::new(),
            workspace_path_scope: None,
            network_domains: Vec::new(),
            max_risk: RiskClass::ReadOnly,
        }
    }
}

impl CapabilityPermissions {
    /// Create new permissions with the specified operations.
    pub fn new(operations: impl IntoIterator<Item = CapabilityOperation>) -> Self {
        Self {
            allowed_operations: operations.into_iter().collect(),
            workspace_path_scope: None,
            network_domains: Vec::new(),
            max_risk: RiskClass::ReadOnly,
        }
    }

    /// Builder to set workspace path boundary.
    pub fn with_path_scope(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace_path_scope = Some(path.into());
        self
    }

    /// Builder to set allowed network domains.
    pub fn with_domains(mut self, domains: Vec<String>) -> Self {
        self.network_domains = domains;
        self
    }

    /// Builder to set maximum risk classification.
    pub fn with_max_risk(mut self, risk: RiskClass) -> Self {
        self.max_risk = risk;
        self
    }

    /// Predefined read-only permission set.
    pub fn read_only() -> Self {
        let mut ops = HashSet::new();
        ops.insert(CapabilityOperation::Read);
        Self {
            allowed_operations: ops,
            workspace_path_scope: None,
            network_domains: Vec::new(),
            max_risk: RiskClass::ReadOnly,
        }
    }

    /// Predefined full-access permission set for trusted local providers.
    pub fn full_access() -> Self {
        let mut ops = HashSet::new();
        ops.insert(CapabilityOperation::Read);
        ops.insert(CapabilityOperation::Write);
        ops.insert(CapabilityOperation::Execute);
        ops.insert(CapabilityOperation::Admin);
        Self {
            allowed_operations: ops,
            workspace_path_scope: None,
            network_domains: Vec::new(),
            max_risk: RiskClass::ExternalCommunication,
        }
    }

    /// Check if the operation is allowed.
    pub fn permits_operation(&self, op: CapabilityOperation) -> bool {
        self.allowed_operations.contains(&op)
    }

    /// Check if target path falls within the permitted workspace scope.
    pub fn permits_path(&self, target_path: &Path) -> bool {
        // Protected paths (.git, .m31a) are never permitted by capability permissions
        for comp in target_path.components() {
            if let std::path::Component::Normal(c) = comp {
                let s = c.to_string_lossy();
                if s.eq_ignore_ascii_case(".git") || s.eq_ignore_ascii_case(".m31a") {
                    return false;
                }
            }
        }

        let Some(scope) = &self.workspace_path_scope else {
            // If no path scope restriction is configured, path access is permitted
            return true;
        };

        // If target is relative and has no root, it is implicitly within scope
        if target_path.is_relative() && !target_path.has_root() {
            return true;
        }

        // Canonicalized or prefix check
        if let (Ok(canon_target), Ok(canon_scope)) =
            (target_path.canonicalize(), scope.canonicalize())
        {
            if let Ok(rel) = canon_target.strip_prefix(&canon_scope) {
                for comp in rel.components() {
                    if let std::path::Component::Normal(c) = comp {
                        let s = c.to_string_lossy();
                        if s.eq_ignore_ascii_case(".git") || s.eq_ignore_ascii_case(".m31a") {
                            return false;
                        }
                    }
                }
            }
            canon_target.starts_with(canon_scope)
        } else {
            target_path.starts_with(scope)
        }
    }

    /// Check if target domain is permitted.
    pub fn permits_domain(&self, domain: &str) -> bool {
        if self.network_domains.is_empty() {
            return false;
        }
        let domain_lower = domain.to_lowercase();
        self.network_domains.iter().any(|d| {
            let d_lower = d.to_lowercase();
            domain_lower == d_lower || domain_lower.ends_with(&format!(".{d_lower}"))
        })
    }

    /// Check if risk level is acceptable.
    pub fn permits_risk(&self, risk: RiskClass) -> bool {
        (risk as u8) <= (self.max_risk as u8)
    }

    /// Validate an action request against permissions, returning `Ok(())` or `Err(CapabilityError::PermissionDenied)`.
    pub fn validate(
        &self,
        op: CapabilityOperation,
        path: Option<&Path>,
        domain: Option<&str>,
        risk: Option<RiskClass>,
    ) -> Result<(), CapabilityError> {
        if !self.permits_operation(op) {
            return Err(CapabilityError::PermissionDenied(format!(
                "operation {op:?} not allowed by capability permissions"
            )));
        }

        if let Some(p) = path
            && !self.permits_path(p)
        {
            return Err(CapabilityError::PathOutOfBounds {
                path: p.display().to_string(),
                workspace: self
                    .workspace_path_scope
                    .as_deref()
                    .map(|w| w.display().to_string())
                    .unwrap_or_else(|| "<unspecified>".to_string()),
            });
        }

        if let Some(d) = domain
            && !self.permits_domain(d)
        {
            return Err(CapabilityError::PermissionDenied(format!(
                "network access to domain {d:?} not allowed by capability permissions"
            )));
        }

        if let Some(r) = risk
            && !self.permits_risk(r)
        {
            return Err(CapabilityError::PermissionDenied(format!(
                "requested risk level {r:?} exceeds maximum permitted {max:?}",
                max = self.max_risk
            )));
        }

        Ok(())
    }

    /// Compute the strict intersection of this permission set with another.
    pub fn intersect(&self, other: &CapabilityPermissions) -> CapabilityPermissions {
        let allowed_operations: HashSet<CapabilityOperation> = self
            .allowed_operations
            .intersection(&other.allowed_operations)
            .copied()
            .collect();

        // Narrowest path scope
        let workspace_path_scope = match (&self.workspace_path_scope, &other.workspace_path_scope) {
            (Some(a), Some(b)) => {
                if a.starts_with(b) {
                    Some(a.clone())
                } else if b.starts_with(a) {
                    Some(b.clone())
                } else {
                    Some(a.clone())
                }
            }
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (None, None) => None,
        };

        // Shared network domains
        let network_domains: Vec<String> = self
            .network_domains
            .iter()
            .filter(|d| other.network_domains.contains(d))
            .cloned()
            .collect();

        // Lowest maximum risk
        let max_risk = if (self.max_risk as u8) < (other.max_risk as u8) {
            self.max_risk
        } else {
            other.max_risk
        };

        CapabilityPermissions {
            allowed_operations,
            workspace_path_scope,
            network_domains,
            max_risk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permissions_operation_check() {
        let perms = CapabilityPermissions::read_only();
        assert!(perms.permits_operation(CapabilityOperation::Read));
        assert!(!perms.permits_operation(CapabilityOperation::Write));
        assert!(!perms.permits_operation(CapabilityOperation::Execute));
    }

    #[test]
    fn test_permissions_path_validation() {
        let perms =
            CapabilityPermissions::new([CapabilityOperation::Read, CapabilityOperation::Write])
                .with_path_scope("/tmp/workspace");
        assert!(perms.permits_path(Path::new("relative/file.txt")));
        assert!(perms.permits_path(Path::new("/tmp/workspace/src/lib.rs")));
        assert!(!perms.permits_path(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_permissions_intersection() {
        let p1 =
            CapabilityPermissions::new([CapabilityOperation::Read, CapabilityOperation::Write])
                .with_max_risk(RiskClass::HighRiskMutation);
        let p2 =
            CapabilityPermissions::new([CapabilityOperation::Read, CapabilityOperation::Execute])
                .with_max_risk(RiskClass::ReadOnly);

        let combined = p1.intersect(&p2);
        assert!(combined.permits_operation(CapabilityOperation::Read));
        assert!(!combined.permits_operation(CapabilityOperation::Write));
        assert!(!combined.permits_operation(CapabilityOperation::Execute));
        assert_eq!(combined.max_risk, RiskClass::ReadOnly);
    }
}
