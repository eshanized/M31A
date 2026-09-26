//! Policy context for execution authorization and sandbox profiles.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Policy context associated with mission execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyContext {
    /// Active policy profile name (e.g. "default", "strict", "ci").
    pub profile_name: String,
    /// Minimum required security level.
    pub security_level: u32,
    /// Explicitly allowed capabilities for the mission.
    pub allowed_capabilities: Vec<String>,
}

impl PolicyContext {
    /// Create a standard default policy context.
    pub fn standard() -> Self {
        Self {
            profile_name: "standard".to_string(),
            security_level: 1,
            allowed_capabilities: vec!["fs:read".to_string(), "git".to_string()],
        }
    }
}

impl Default for PolicyContext {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_context_standard() {
        let ctx = PolicyContext::standard();
        assert_eq!(ctx.profile_name, "standard");
        assert_eq!(ctx.security_level, 1);
    }

    #[test]
    fn test_policy_context_serde_roundtrip() {
        let ctx = PolicyContext {
            profile_name: "strict".to_string(),
            security_level: 2,
            allowed_capabilities: vec!["fs:read".to_string()],
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: PolicyContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, parsed);
    }
}
