//! 10-tier policy layer hierarchy and non-weakening decision merger (POL-03, POL-06, D-02).

use serde::{Deserialize, Serialize};

use crate::kernel::seams::policy::PolicyDecision;

/// 10-tier policy layer hierarchy ordered by authority precedence.
/// Lower numerical value indicates higher authority:
/// BuiltInSafety (0) has highest authority; DeveloperDefault (9) has lowest authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PolicyLayer {
    BuiltInSafety = 0,    // Absolute vetoes, immutable
    SystemAdmin = 1,      // /etc/m31/policy.toml
    Organization = 2,     // Enterprise / team compliance
    Workspace = 3,        // <workspace>/.m31/policy.toml
    User = 4,             // ~/.config/m31/policy.toml
    Mission = 5,          // Mission PolicyContext
    AgentRole = 6,        // AgentRoleProfile bounds
    Task = 7,             // Task-specific constraints
    SessionApproval = 8,  // Durable SQLite policy_grants
    DeveloperDefault = 9, // POL-04 built-in fallback
}

impl PolicyLayer {
    /// Precedence rank from 0 (highest) to 9 (lowest).
    pub fn precedence_rank(self) -> u32 {
        self as u32
    }

    /// Descriptive name of the policy layer.
    pub fn name(self) -> &'static str {
        match self {
            Self::BuiltInSafety => "built_in_safety",
            Self::SystemAdmin => "system_admin",
            Self::Organization => "organization",
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::Mission => "mission",
            Self::AgentRole => "agent_role",
            Self::Task => "task",
            Self::SessionApproval => "session_approval",
            Self::DeveloperDefault => "developer_default",
        }
    }

    /// Human-readable authority source string for audit provenance.
    pub fn authority_source(self) -> &'static str {
        match self {
            Self::BuiltInSafety => "built-in safety invariant",
            Self::SystemAdmin => "system policy (/etc/m31/policy.toml)",
            Self::Organization => "organization policy",
            Self::Workspace => "workspace policy (<workspace>/.m31/policy.toml)",
            Self::User => "user policy (~/.config/m31/policy.toml)",
            Self::Mission => "mission policy context",
            Self::AgentRole => "agent role profile",
            Self::Task => "task constraints",
            Self::SessionApproval => "session approval grant",
            Self::DeveloperDefault => "developer default fallback",
        }
    }
}

/// Merges decisions from a higher-authority layer and a lower-authority layer.
///
/// Mathematical Invariants (POL-06, Law 3):
/// 1. A `Deny` from higher authority is an absolute immutable veto that cannot be overridden.
/// 2. Lower layers may further restrict permissions (Allow -> Ask -> Deny), but cannot weaken them.
/// 3. SessionApproval (Layer 8) can resolve an `Ask` to `Allow` within its explicit bounded grant.
pub fn merge_preliminary_decision(
    higher: (PolicyDecision, PolicyLayer, String),
    lower: (PolicyDecision, PolicyLayer, String),
) -> (PolicyDecision, PolicyLayer, String) {
    let (high_dec, high_layer, high_rule) = higher;
    let (low_dec, low_layer, low_rule) = lower;

    // Invariant 1 (POL-06, Law 3): DENY from higher authority is an absolute immutable veto
    if high_dec == PolicyDecision::Deny {
        return (high_dec, high_layer, high_rule);
    }

    // Invariant 2: Lower layer may further restrict (ALLOW -> ASK -> DENY), but cannot weaken
    match (high_dec, low_dec) {
        (PolicyDecision::Allow, PolicyDecision::Deny) => (low_dec, low_layer, low_rule),
        (PolicyDecision::Allow, PolicyDecision::Ask) => (low_dec, low_layer, low_rule),
        (PolicyDecision::Ask, PolicyDecision::Deny) => (low_dec, low_layer, low_rule),
        // Layer 8 Session Approval can resolve an ASK if within exact bounded grant
        (PolicyDecision::Ask, PolicyDecision::Allow)
            if low_layer == PolicyLayer::SessionApproval =>
        {
            (low_dec, low_layer, low_rule)
        }
        _ => (high_dec, high_layer, high_rule),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_ranking_order() {
        assert!(PolicyLayer::BuiltInSafety < PolicyLayer::SystemAdmin);
        assert!(PolicyLayer::SystemAdmin < PolicyLayer::Organization);
        assert!(PolicyLayer::Organization < PolicyLayer::Workspace);
        assert!(PolicyLayer::Workspace < PolicyLayer::User);
        assert!(PolicyLayer::User < PolicyLayer::Mission);
        assert!(PolicyLayer::Mission < PolicyLayer::AgentRole);
        assert!(PolicyLayer::AgentRole < PolicyLayer::Task);
        assert!(PolicyLayer::Task < PolicyLayer::SessionApproval);
        assert!(PolicyLayer::SessionApproval < PolicyLayer::DeveloperDefault);
    }

    #[test]
    fn test_built_in_deny_is_immutable_veto() {
        let higher = (
            PolicyDecision::Deny,
            PolicyLayer::BuiltInSafety,
            "veto-cred".to_string(),
        );
        let lower = (
            PolicyDecision::Allow,
            PolicyLayer::Workspace,
            "allow-all".to_string(),
        );
        let merged = merge_preliminary_decision(higher, lower);
        assert_eq!(merged.0, PolicyDecision::Deny);
        assert_eq!(merged.1, PolicyLayer::BuiltInSafety);
        assert_eq!(merged.2, "veto-cred");
    }

    #[test]
    fn test_lower_layer_can_further_restrict() {
        let higher = (
            PolicyDecision::Allow,
            PolicyLayer::SystemAdmin,
            "sys-allow".to_string(),
        );
        let lower = (
            PolicyDecision::Deny,
            PolicyLayer::Workspace,
            "ws-deny".to_string(),
        );
        let merged = merge_preliminary_decision(higher, lower);
        assert_eq!(merged.0, PolicyDecision::Deny);
        assert_eq!(merged.1, PolicyLayer::Workspace);
        assert_eq!(merged.2, "ws-deny");
    }

    #[test]
    fn test_lower_layer_cannot_weaken_deny() {
        let higher = (
            PolicyDecision::Deny,
            PolicyLayer::Organization,
            "org-deny".to_string(),
        );
        let lower = (
            PolicyDecision::Allow,
            PolicyLayer::User,
            "user-allow".to_string(),
        );
        let merged = merge_preliminary_decision(higher, lower);
        assert_eq!(merged.0, PolicyDecision::Deny);
        assert_eq!(merged.1, PolicyLayer::Organization);
    }

    #[test]
    fn test_session_approval_can_resolve_ask() {
        let higher = (
            PolicyDecision::Ask,
            PolicyLayer::Workspace,
            "ws-ask".to_string(),
        );
        let lower = (
            PolicyDecision::Allow,
            PolicyLayer::SessionApproval,
            "grant-1".to_string(),
        );
        let merged = merge_preliminary_decision(higher, lower);
        assert_eq!(merged.0, PolicyDecision::Allow);
        assert_eq!(merged.1, PolicyLayer::SessionApproval);
    }
}
