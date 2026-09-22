//! TOML Merging & Monotonic Security Accumulation (CFG-01, D-12, THREAT-05).

use std::collections::HashSet;

/// Configuration errors during loading, merging, and validation.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("Security downgrade denied: {reason}")]
    SecurityDowngradeDenied { reason: String },
    #[error("TOML parse error: {0}")]
    TomlParseError(String),
    #[error("Config validation error: {0}")]
    ValidationError(String),
    #[error("Profile '{0}' not found")]
    ProfileNotFound(String),
    #[error("Cyclic inheritance detected in profile: {0}")]
    CyclicProfileInheritance(String),
    #[error("IO error: {0}")]
    IoError(String),
}

impl From<crate::config::schema::ConfigValidationError> for ConfigError {
    fn from(err: crate::config::schema::ConfigValidationError) -> Self {
        ConfigError::ValidationError(err.to_string())
    }
}

/// Recursively merges overlay TOML value into base TOML value.
///
/// Rules:
/// - Tables: recursively merge keys.
/// - Arrays: replace by default per D-12.
/// - Scalars: overlay replaces base.
pub fn deep_merge_toml(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (k, v) in overlay_table {
                if k == "test_command" {
                    base_table.remove("tier3_tests");
                } else if k == "tier3_tests" {
                    base_table.remove("test_command");
                } else if k == "interactive_approvals" {
                    base_table.remove("require_approval_for_destructive");
                } else if k == "require_approval_for_destructive" {
                    base_table.remove("interactive_approvals");
                } else if k == "manifest_file" {
                    base_table.remove("tier1_manifest");
                } else if k == "tier1_manifest" {
                    base_table.remove("manifest_file");
                } else if k == "compiler_command" {
                    base_table.remove("tier2_compiler");
                } else if k == "tier2_compiler" {
                    base_table.remove("compiler_command");
                } else if k == "linter_command" {
                    base_table.remove("tier4_linter");
                } else if k == "tier4_linter" {
                    base_table.remove("linter_command");
                }

                if k == "denied_tools"
                    && let (Some(toml::Value::Array(b_arr)), toml::Value::Array(o_arr)) =
                        (base_table.get_mut("denied_tools"), &v)
                {
                    let mut set: HashSet<String> = b_arr
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                    for item in o_arr {
                        if let Some(s) = item.as_str()
                            && set.insert(s.to_string())
                        {
                            b_arr.push(item.clone());
                        }
                    }
                    continue;
                }

                if let Some(existing) = base_table.get_mut(&k) {
                    deep_merge_toml(existing, v);
                } else {
                    base_table.insert(k, v);
                }
            }
        }
        (base_slot, overlay_val) => {
            *base_slot = overlay_val;
        }
    }
}

/// Monotonic security restriction validator ensuring lower tiers cannot weaken security policies.
pub struct MonotonicSecurityMerger;

impl MonotonicSecurityMerger {
    /// Verify that an incoming overlay layer does not downgrade security invariants established by base.
    pub fn check(base: &toml::Value, overlay: &toml::Value) -> Result<(), ConfigError> {
        // 1. Inspect [policy] section
        let base_policy = base.get("policy").and_then(|p| p.as_table());
        let overlay_policy = overlay.get("policy").and_then(|p| p.as_table());
        if let (Some(bp), Some(op)) = (base_policy, overlay_policy) {
            // interactive_approvals / require_approval_for_destructive: cannot turn true into false
            let check_approval = |table: &toml::map::Map<String, toml::Value>| {
                table
                    .get("interactive_approvals")
                    .or_else(|| table.get("require_approval_for_destructive"))
                    .and_then(|v| v.as_bool())
            };
            if let (Some(true), Some(false)) = (check_approval(bp), check_approval(op)) {
                return Err(ConfigError::SecurityDowngradeDenied {
                    reason:
                        "Cannot disable 'require_approval_for_destructive' / 'interactive_approvals' required by higher precedence tier"
                            .to_string(),
                });
            }

            // default_action: deny > ask > allow
            if let (Some(b_act), Some(o_act)) = (
                bp.get("default_action").and_then(|v| v.as_str()),
                op.get("default_action").and_then(|v| v.as_str()),
            ) {
                let rank = |act: &str| match act.to_lowercase().as_str() {
                    "deny" => 3,
                    "ask" => 2,
                    "allow" => 1,
                    _ => 0,
                };
                if rank(o_act) < rank(b_act) {
                    return Err(ConfigError::SecurityDowngradeDenied {
                        reason: format!(
                            "Cannot relax 'default_action' from '{}' to '{}'",
                            b_act, o_act
                        ),
                    });
                }
            }

            // sandbox_mode: strict > standard > permissive > disabled
            if let (Some(b_sb), Some(o_sb)) = (
                bp.get("sandbox_mode").and_then(|v| v.as_str()),
                op.get("sandbox_mode").and_then(|v| v.as_str()),
            ) {
                let rank = |sb: &str| match sb.to_lowercase().as_str() {
                    "strict" => 4,
                    "standard" => 3,
                    "permissive" => 2,
                    "disabled" | "none" => 1,
                    _ => 0,
                };
                if rank(o_sb) < rank(b_sb) {
                    return Err(ConfigError::SecurityDowngradeDenied {
                        reason: format!(
                            "Cannot relax 'sandbox_mode' from '{}' to '{}'",
                            b_sb, o_sb
                        ),
                    });
                }
            }

            // denied_tools: tools denied in base cannot be removed in overlay
            if let (Some(b_tools), Some(o_tools)) = (
                bp.get("denied_tools").and_then(|v| v.as_array()),
                op.get("denied_tools").and_then(|v| v.as_array()),
            ) {
                let b_set: HashSet<_> = b_tools.iter().filter_map(|v| v.as_str()).collect();
                let o_set: HashSet<_> = o_tools.iter().filter_map(|v| v.as_str()).collect();
                for denied in &b_set {
                    if !o_set.contains(denied) {
                        return Err(ConfigError::SecurityDowngradeDenied {
                            reason: format!(
                                "Cannot remove tool '{}' from denied_tools list",
                                denied
                            ),
                        });
                    }
                }
            }
        }

        // 2. Inspect [runtime] section for sandbox_mode
        let base_rt_sb = base
            .get("runtime")
            .and_then(|r| r.get("sandbox_mode"))
            .and_then(|v| v.as_str());
        let overlay_rt_sb = overlay
            .get("runtime")
            .and_then(|r| r.get("sandbox_mode"))
            .and_then(|v| v.as_str());

        if let (Some(b_sb), Some(o_sb)) = (base_rt_sb, overlay_rt_sb) {
            let rank = |sb: &str| match sb.to_lowercase().as_str() {
                "strict" => 4,
                "standard" => 3,
                "permissive" => 2,
                "disabled" | "none" => 1,
                _ => 0,
            };
            if rank(o_sb) < rank(b_sb) {
                return Err(ConfigError::SecurityDowngradeDenied {
                    reason: format!(
                        "Cannot relax runtime 'sandbox_mode' from '{}' to '{}'",
                        b_sb, o_sb
                    ),
                });
            }
        }

        // 3. Inspect capabilities monotonicity: child cannot expand capabilities beyond parent
        let base_caps = base.get("capabilities").and_then(|c| c.as_array());
        let overlay_caps = overlay.get("capabilities").and_then(|c| c.as_array());
        if let (Some(bc), Some(oc)) = (base_caps, overlay_caps) {
            let b_set: HashSet<_> = bc.iter().filter_map(|v| v.as_str()).collect();
            let o_set: HashSet<_> = oc.iter().filter_map(|v| v.as_str()).collect();
            if overlay.get("extends").is_some() && !b_set.is_empty() {
                for cap in &o_set {
                    if !b_set.contains(cap) {
                        return Err(ConfigError::SecurityDowngradeDenied {
                            reason: format!(
                                "Child profile cannot add capability '{}' not permitted by parent profile",
                                cap
                            ),
                        });
                    }
                }
            }
        }

        // 4. Inspect verification_tier monotonicity: child cannot lower verification tier
        let base_vt = base.get("verification_tier").and_then(|v| v.as_integer());
        let overlay_vt = overlay
            .get("verification_tier")
            .and_then(|v| v.as_integer());
        if let (Some(b_tier), Some(o_tier)) = (base_vt, overlay_vt)
            && o_tier < b_tier
        {
            return Err(ConfigError::SecurityDowngradeDenied {
                reason: format!(
                    "Child profile cannot lower verification_tier from {} to {}",
                    b_tier, o_tier
                ),
            });
        }

        Ok(())
    }
}
