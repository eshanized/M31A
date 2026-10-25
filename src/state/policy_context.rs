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
            allowed_capabilities: vec![
                "workspace_fs_read".to_string(),
                "repo_index".to_string(),
                "fs:read".to_string(),
                "git".to_string(),
            ],
        }
    }

    /// Resolve policy context from an authoritative resolved profile name.
    pub fn for_profile(profile_name: &str) -> Result<Self, crate::config::ConfigError> {
        let resolver = crate::config::ProfileResolver::with_canonical_profiles();
        let profile_val = resolver.resolve_profile(profile_name)?;

        let security_level = profile_val
            .get("verification_tier")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as u32;

        let allowed_capabilities = profile_val
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            profile_name: profile_name.to_string(),
            security_level,
            allowed_capabilities,
        })
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
