//! Dynamic feature flag registry, staged rollouts, and dependency normalization.
//!
//! Controls experimental capabilities, gated features, and compatibility modes across releases.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// High-level lifecycle stage for a feature flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStage {
    UnderDevelopment,
    Experimental {
        name: &'static str,
        menu_description: &'static str,
    },
    Stable,
    Deprecated,
    Removed,
}

/// Known feature keys controlling engine capabilities and security policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKey {
    // Stable capabilities
    UnifiedExec,
    UnifiedExecTty,
    ApplyPatchStreaming,
    Arg0Dispatch,
    WorktreeManagement,
    PorcelainGit,
    ProcessHardening,
    TerminalDetection,
    Analytics,
    ExternalAgentMigration,
    AwsAuthSigV4,
    RealtimeVoice,
    ConnectorsPolicy,
    DescriptorPinning,
    HookOutputSpilling,
    HierarchicalInstructions,
    ContextCompression,
    CheckpointRevert,
    PathLocking,
    LocalLogsProtection,

    // Experimental capabilities
    CodeMode,
    CodeModeHost,
    CodeModeInterrupt,
    GuardianV2,
    GuardianV3,
    MultiAgentV2,
    NetworkProxy,
    RemoteMessageBoard,
    AsyncCompaction,
    SleepTool,
    LocalModelInference,
    RecapCatchupPrompt,

    // Under-development capabilities
    AutonomousSelfEvolvingAgent,
}

impl FeatureKey {
    pub const ALL: &'static [FeatureKey] = &[
        FeatureKey::UnifiedExec,
        FeatureKey::UnifiedExecTty,
        FeatureKey::ApplyPatchStreaming,
        FeatureKey::Arg0Dispatch,
        FeatureKey::WorktreeManagement,
        FeatureKey::PorcelainGit,
        FeatureKey::ProcessHardening,
        FeatureKey::TerminalDetection,
        FeatureKey::Analytics,
        FeatureKey::ExternalAgentMigration,
        FeatureKey::AwsAuthSigV4,
        FeatureKey::RealtimeVoice,
        FeatureKey::ConnectorsPolicy,
        FeatureKey::DescriptorPinning,
        FeatureKey::HookOutputSpilling,
        FeatureKey::HierarchicalInstructions,
        FeatureKey::ContextCompression,
        FeatureKey::CheckpointRevert,
        FeatureKey::PathLocking,
        FeatureKey::LocalLogsProtection,
        FeatureKey::CodeMode,
        FeatureKey::CodeModeHost,
        FeatureKey::CodeModeInterrupt,
        FeatureKey::GuardianV2,
        FeatureKey::GuardianV3,
        FeatureKey::MultiAgentV2,
        FeatureKey::NetworkProxy,
        FeatureKey::RemoteMessageBoard,
        FeatureKey::AsyncCompaction,
        FeatureKey::SleepTool,
        FeatureKey::LocalModelInference,
        FeatureKey::RecapCatchupPrompt,
        FeatureKey::AutonomousSelfEvolvingAgent,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            FeatureKey::UnifiedExec => "unified_exec",
            FeatureKey::UnifiedExecTty => "unified_exec_tty",
            FeatureKey::ApplyPatchStreaming => "apply_patch_streaming",
            FeatureKey::Arg0Dispatch => "arg0_dispatch",
            FeatureKey::WorktreeManagement => "worktree_management",
            FeatureKey::PorcelainGit => "porcelain_git",
            FeatureKey::ProcessHardening => "process_hardening",
            FeatureKey::TerminalDetection => "terminal_detection",
            FeatureKey::Analytics => "analytics",
            FeatureKey::ExternalAgentMigration => "external_agent_migration",
            FeatureKey::AwsAuthSigV4 => "aws_auth_sigv4",
            FeatureKey::RealtimeVoice => "realtime_voice",
            FeatureKey::ConnectorsPolicy => "connectors_policy",
            FeatureKey::DescriptorPinning => "descriptor_pinning",
            FeatureKey::HookOutputSpilling => "hook_output_spilling",
            FeatureKey::HierarchicalInstructions => "hierarchical_instructions",
            FeatureKey::ContextCompression => "context_compression",
            FeatureKey::CheckpointRevert => "checkpoint_revert",
            FeatureKey::PathLocking => "path_locking",
            FeatureKey::LocalLogsProtection => "local_logs_protection",
            FeatureKey::CodeMode => "code_mode",
            FeatureKey::CodeModeHost => "code_mode_host",
            FeatureKey::CodeModeInterrupt => "code_mode_interrupt",
            FeatureKey::GuardianV2 => "guardian_v2",
            FeatureKey::GuardianV3 => "guardian_v3",
            FeatureKey::MultiAgentV2 => "multi_agent_v2",
            FeatureKey::NetworkProxy => "network_proxy",
            FeatureKey::RemoteMessageBoard => "remote_message_board",
            FeatureKey::AsyncCompaction => "async_compaction",
            FeatureKey::SleepTool => "sleep_tool",
            FeatureKey::LocalModelInference => "local_model_inference",
            FeatureKey::RecapCatchupPrompt => "recap_catchup_prompt",
            FeatureKey::AutonomousSelfEvolvingAgent => "autonomous_self_evolving_agent",
        }
    }

    pub fn from_str_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_str() == name)
    }

    pub fn default_stage(&self) -> FeatureStage {
        match self {
            FeatureKey::UnifiedExec
            | FeatureKey::UnifiedExecTty
            | FeatureKey::ApplyPatchStreaming
            | FeatureKey::Arg0Dispatch
            | FeatureKey::WorktreeManagement
            | FeatureKey::PorcelainGit
            | FeatureKey::ProcessHardening
            | FeatureKey::TerminalDetection
            | FeatureKey::Analytics
            | FeatureKey::ExternalAgentMigration
            | FeatureKey::AwsAuthSigV4
            | FeatureKey::ConnectorsPolicy
            | FeatureKey::DescriptorPinning
            | FeatureKey::HookOutputSpilling
            | FeatureKey::HierarchicalInstructions
            | FeatureKey::ContextCompression
            | FeatureKey::CheckpointRevert
            | FeatureKey::PathLocking
            | FeatureKey::LocalLogsProtection => FeatureStage::Stable,

            FeatureKey::CodeMode => FeatureStage::Experimental {
                name: "JavaScript Code Mode",
                menu_description: "Enable JavaScript code mode backed by runtime runner.",
            },
            FeatureKey::CodeModeHost => FeatureStage::Experimental {
                name: "Code Mode Host Process",
                menu_description: "Run code mode execution in isolated background host process.",
            },
            FeatureKey::CodeModeInterrupt => FeatureStage::Experimental {
                name: "Code Mode Cell Interrupt",
                menu_description: "Cancel active code mode loops when session is cancelled.",
            },
            FeatureKey::GuardianV2 => FeatureStage::Experimental {
                name: "Guardian Safety V2",
                menu_description: "Multi-layered tool call safety policy evaluation.",
            },
            FeatureKey::GuardianV3 => FeatureStage::Experimental {
                name: "Guardian Safety V3",
                menu_description: "Dynamic proactive tool approval and taint tracking.",
            },
            FeatureKey::MultiAgentV2 => FeatureStage::Experimental {
                name: "Multi-Agent Topology V2",
                menu_description: "Parent-child agent hierarchies and shared message board.",
            },
            FeatureKey::NetworkProxy => FeatureStage::Experimental {
                name: "Strict Network Proxy",
                menu_description: "Forward external tool traffic through proxy policy filter.",
            },
            FeatureKey::RemoteMessageBoard => FeatureStage::Experimental {
                name: "Remote Message Board",
                menu_description: "Sync inter-agent message board over distributed storage.",
            },
            FeatureKey::AsyncCompaction => FeatureStage::Experimental {
                name: "Background Compaction",
                menu_description: "Asynchronous context window summarization and pruning.",
            },
            FeatureKey::SleepTool => FeatureStage::Experimental {
                name: "Sleep / Wait Tool",
                menu_description: "Periodic wait and execution poll tool.",
            },
            FeatureKey::LocalModelInference => FeatureStage::Experimental {
                name: "Local Model Inference",
                menu_description: "Direct native integration with Ollama and LM Studio.",
            },
            FeatureKey::RealtimeVoice => FeatureStage::Experimental {
                name: "Realtime Voice Controls",
                menu_description: "Realtime WebRTC audio control protocol and peak meter.",
            },
            FeatureKey::RecapCatchupPrompt => FeatureStage::Experimental {
                name: "Context Recap Catchup Prompt",
                menu_description: "Structured returner catch-up prompt with 40-80 word summary and next action.",
            },
            FeatureKey::AutonomousSelfEvolvingAgent => FeatureStage::UnderDevelopment,
        }
    }

    pub fn is_enabled_by_default(&self) -> bool {
        matches!(self.default_stage(), FeatureStage::Stable)
    }
}

/// Dynamic feature registry with config overrides and runtime toggles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureRegistry {
    overrides: BTreeMap<FeatureKey, bool>,
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self {
            overrides: BTreeMap::new(),
        }
    }

    /// Evaluates whether a feature is currently enabled.
    pub fn is_enabled(&self, key: FeatureKey) -> bool {
        if let Some(&overridden) = self.overrides.get(&key) {
            return overridden;
        }

        key.is_enabled_by_default()
    }

    /// Explicitly override a feature flag state.
    pub fn set_enabled(&mut self, key: FeatureKey, enabled: bool) {
        self.overrides.insert(key, enabled);
    }

    /// Reset an override back to default.
    pub fn reset_override(&mut self, key: FeatureKey) {
        self.overrides.remove(&key);
    }

    /// Returns all currently active features.
    pub fn active_features(&self) -> BTreeSet<FeatureKey> {
        FeatureKey::ALL
            .iter()
            .copied()
            .filter(|&k| self.is_enabled(k))
            .collect()
    }

    /// Lists experimental features available for toggle menus.
    pub fn list_experimental_features(
        &self,
    ) -> Vec<(FeatureKey, &'static str, &'static str, bool)> {
        FeatureKey::ALL
            .iter()
            .copied()
            .filter_map(|k| match k.default_stage() {
                FeatureStage::Experimental {
                    name,
                    menu_description,
                } => Some((k, name, menu_description, self.is_enabled(k))),
                _ => None,
            })
            .collect()
    }

    /// Loads feature overrides from a TOML configuration table (`[features]`).
    /// Supports both simple booleans (`key = true`) and table format (`key = { enabled = true }`).
    /// Legacy keys are aliased to their successor with a deprecation warning
    /// (report §7.1, upstream `record_legacy_usage`); unknown keys are ignored
    /// with a warning so config typos never fail silently.
    pub fn load_from_toml_table(&mut self, table: &toml::Table) {
        for (key_str, val) in table {
            let enabled_opt = if let Some(b) = val.as_bool() {
                Some(b)
            } else if let Some(sub_table) = val.as_table() {
                sub_table.get("enabled").and_then(|v| v.as_bool())
            } else {
                None
            };

            if let Some(key) = FeatureKey::from_str_name(key_str) {
                if let Some(enabled) = enabled_opt {
                    self.set_enabled(key, enabled);
                } else {
                    tracing::warn!(
                        "Feature `{key_str}` has an unsupported value type; expected a boolean or `enabled = bool` table."
                    );
                }
            } else if let Some((successor, successor_name)) = resolve_legacy_feature_key(key_str) {
                if let Some(enabled) = enabled_opt {
                    self.set_enabled(successor, enabled);
                }
                tracing::warn!(
                    "Legacy feature key `{key_str}` is deprecated; honoring it as `{successor_name}`. Update your [features] config to use `{successor_name}`."
                );
            } else {
                tracing::warn!(
                    "Unknown feature key `{key_str}` in [features] config (no matching FeatureKey; ignored)."
                );
            }
        }
    }

    /// Syncs feature overrides from a map (e.g. from `Config.features`).
    pub fn sync_from_map(&mut self, map: &std::collections::HashMap<String, bool>) {
        for (k, v) in map {
            if let Some(key) = FeatureKey::from_str_name(k) {
                self.set_enabled(key, *v);
            }
        }
    }

    /// Exports active overrides to a key-value map.
    pub fn sync_to_map(&self, map: &mut std::collections::HashMap<String, bool>) {
        for (k, v) in &self.overrides {
            map.insert(k.as_str().to_string(), *v);
        }
    }
}

/// Resolves a legacy (pre-cleanup) feature key to its real successor
/// (report §7.1). Returns the successor key and its canonical name so callers
/// can emit a user-facing deprecation notice. Legacy keys without a real
/// successor are intentionally NOT aliased — they warn as unknown instead.
pub fn resolve_legacy_feature_key(legacy: &str) -> Option<(FeatureKey, &'static str)> {
    let resolved = match legacy {
        "guardian_approval" => (FeatureKey::GuardianV2, "guardian_v2"),
        "agent_message_board" => (FeatureKey::RemoteMessageBoard, "remote_message_board"),
        "worktrees" => (FeatureKey::WorktreeManagement, "worktree_management"),
        "code_mode_tool_search" => (FeatureKey::CodeMode, "code_mode"),
        "collab" => (FeatureKey::MultiAgentV2, "multi_agent_v2"),
        _ => return None,
    };
    Some(resolved)
}

/// Emits a warning if any `UnderDevelopment` features are active unless explicitly suppressed.
/// Emits telemetry warning when unstable or experimental features are enabled.
pub fn unstable_features_warning_event(
    registry: &FeatureRegistry,
    suppress_unstable_features_warning: bool,
    config_path: &str,
) -> Option<String> {
    if suppress_unstable_features_warning {
        return None;
    }

    let mut under_dev_keys = Vec::new();
    for &key in FeatureKey::ALL {
        if registry.is_enabled(key) && matches!(key.default_stage(), FeatureStage::UnderDevelopment)
        {
            under_dev_keys.push(key.as_str());
        }
    }

    if under_dev_keys.is_empty() {
        return None;
    }

    under_dev_keys.sort();
    Some(format!(
        "Under-development features enabled: {}. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in {}.",
        under_dev_keys.join(", "),
        config_path
    ))
}

/// §7.1: `FeatureToml<T>` = bool **or** per-feature config struct.
/// Deserializes feature flag tables from configuration files.
/// with arbitrary nested configuration (not just `enabled`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FeatureToml<T> {
    Simple(bool),
    Config(T),
}

impl<T> FeatureToml<T> {
    /// Extracts the enabled state (true for Config variant unless T has
    /// an explicit `enabled` field — caller handles nested extraction).
    pub fn enabled(&self) -> bool {
        match self {
            FeatureToml::Simple(b) => *b,
            FeatureToml::Config(_) => true,
        }
    }
}

/// Normalizes feature dependencies across feature tiers (e.g., `CodeModeOnly` ⇒ `CodeMode`,
/// `GuardianV3` ⇒ `GuardianV2`) to ensure prerequisite features are consistently enabled.
pub fn normalize_dependencies(enabled: &mut BTreeSet<FeatureKey>) {
    // CodeModeOnly implies CodeMode
    if enabled.remove(&FeatureKey::CodeModeHost) || enabled.remove(&FeatureKey::CodeModeInterrupt) {
        enabled.insert(FeatureKey::CodeMode);
    }
    // GuardianV3 implies GuardianV2
    if enabled.remove(&FeatureKey::GuardianV3) {
        enabled.insert(FeatureKey::GuardianV2);
    }
    // MultiAgentV2 implies GuardianV2
    if enabled.contains(&FeatureKey::MultiAgentV2) {
        enabled.insert(FeatureKey::GuardianV2);
    }
    // NetworkProxy implies TerminalDetection
    if enabled.contains(&FeatureKey::NetworkProxy) {
        enabled.insert(FeatureKey::TerminalDetection);
    }
}

/// §7.1: in-app announcement configuration (upstream `announcement_tip.toml`).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FeatureAnnouncement {
    /// ISO 8601 date when the announcement becomes visible.
    pub visible_from: Option<String>,
    /// ISO 8601 date when the announcement expires.
    pub visible_until: Option<String>,
    /// Minimum version (semver) required to show the announcement.
    pub min_version: Option<String>,
    /// Maximum version (semver) before the announcement is hidden.
    pub max_version: Option<String>,
    /// The announcement title.
    pub title: String,
    /// The announcement body (markdown supported).
    pub body: String,
    /// Optional action button (label, action_type, payload).
    pub action: Option<(String, String, String)>,
}

impl FeatureAnnouncement {
    /// Evaluates whether this announcement should be shown given the current
    /// date and version.
    pub fn is_visible(&self, now: &chrono::DateTime<chrono::Utc>, current_version: &str) -> bool {
        if let Some(from) = &self.visible_from
            && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(from)
            && now < &dt.with_timezone(&chrono::Utc)
        {
            return false;
        }
        if let Some(until) = &self.visible_until
            && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until)
            && now > &dt.with_timezone(&chrono::Utc)
        {
            return false;
        }
        if let Some(min) = &self.min_version
            && version_compare(current_version, min).is_lt()
        {
            return false;
        }
        if let Some(max) = &self.max_version
            && version_compare(current_version, max).is_ge()
        {
            return false;
        }
        true
    }
}

/// Semver-aware comparison (simplified; delegates to `semver` crate if available).
fn version_compare(a: &str, b: &str) -> std::cmp::Ordering {
    use semver::Version;
    let va = Version::parse(a).ok();
    let vb = Version::parse(b).ok();
    match (va, vb) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.cmp(b), // Fallback to string compare
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_defaults() {
        let registry = FeatureRegistry::new();
        assert!(registry.is_enabled(FeatureKey::UnifiedExec));
        assert!(registry.is_enabled(FeatureKey::ProcessHardening));
        assert!(registry.is_enabled(FeatureKey::Analytics));
        assert!(registry.is_enabled(FeatureKey::DescriptorPinning));
        assert!(registry.is_enabled(FeatureKey::HookOutputSpilling));
        assert!(registry.is_enabled(FeatureKey::HierarchicalInstructions));
        assert!(registry.is_enabled(FeatureKey::CheckpointRevert));
        assert!(!registry.is_enabled(FeatureKey::CodeMode)); // Experimental default false
        assert!(!registry.is_enabled(FeatureKey::AutonomousSelfEvolvingAgent)); // UnderDevelopment default false
    }

    #[test]
    fn test_feature_overrides() {
        let mut registry = FeatureRegistry::new();
        assert!(!registry.is_enabled(FeatureKey::CodeMode));

        registry.set_enabled(FeatureKey::CodeMode, true);
        assert!(registry.is_enabled(FeatureKey::CodeMode));

        registry.set_enabled(FeatureKey::UnifiedExec, false);
        assert!(!registry.is_enabled(FeatureKey::UnifiedExec));

        registry.reset_override(FeatureKey::CodeMode);
        assert!(!registry.is_enabled(FeatureKey::CodeMode));
    }

    #[test]
    fn test_load_from_toml_table() {
        let toml_str = r#"
            code_mode = true
            checkpoint_revert = false
            recap_catchup_prompt = { enabled = true }
        "#;
        let table: toml::Table = toml::from_str(toml_str).expect("parse toml");
        let mut registry = FeatureRegistry::new();
        registry.load_from_toml_table(&table);

        assert!(registry.is_enabled(FeatureKey::CodeMode));
        assert!(!registry.is_enabled(FeatureKey::CheckpointRevert));
        assert!(registry.is_enabled(FeatureKey::RecapCatchupPrompt));
    }

    #[test]
    fn test_load_from_toml_table_unknown_keys_ignored_with_warning() {
        let toml_str = r#"
            code_mode = true
            guardian_approval = true
            not_a_real_feature = false
        "#;
        let table: toml::Table = toml::from_str(toml_str).expect("parse toml");
        let mut registry = FeatureRegistry::new();
        // Must not panic on unknown keys; known keys still applied.
        registry.load_from_toml_table(&table);
        assert!(registry.is_enabled(FeatureKey::CodeMode));
    }

    #[test]
    fn test_legacy_feature_key_aliases() {
        // Legacy keys resolve to their real successor (report §7.1).
        assert_eq!(
            resolve_legacy_feature_key("guardian_approval").map(|(k, _)| k),
            Some(FeatureKey::GuardianV2)
        );
        assert_eq!(
            resolve_legacy_feature_key("agent_message_board").map(|(k, _)| k),
            Some(FeatureKey::RemoteMessageBoard)
        );
        assert_eq!(
            resolve_legacy_feature_key("worktrees").map(|(k, _)| k),
            Some(FeatureKey::WorktreeManagement)
        );
        // No invented alias for keys without a real successor.
        assert!(resolve_legacy_feature_key("bwrap_sandbox").is_none());
        assert!(resolve_legacy_feature_key("streaming").is_none());

        // Loading a legacy key applies the successor's value.
        let toml_str = r#"
            guardian_approval = true
            worktrees = false
        "#;
        let table: toml::Table = toml::from_str(toml_str).expect("parse toml");
        let mut registry = FeatureRegistry::new();
        registry.load_from_toml_table(&table);
        assert!(registry.is_enabled(FeatureKey::GuardianV2));
        assert!(!registry.is_enabled(FeatureKey::WorktreeManagement));
    }

    #[test]
    fn test_unstable_features_warning() {
        let mut registry = FeatureRegistry::new();
        // Initially no under-development features enabled
        assert!(
            unstable_features_warning_event(&registry, false, "~/.config/m31a/config.toml")
                .is_none()
        );

        // Enable under-development feature
        registry.set_enabled(FeatureKey::AutonomousSelfEvolvingAgent, true);
        let warn = unstable_features_warning_event(&registry, false, "~/.config/m31a/config.toml");
        assert!(warn.is_some());
        let msg = warn.unwrap();
        assert!(msg.contains("autonomous_self_evolving_agent"));
        assert!(msg.contains("suppress_unstable_features_warning = true"));

        // Suppressed
        assert!(
            unstable_features_warning_event(&registry, true, "~/.config/m31a/config.toml")
                .is_none()
        );
    }

    #[test]
    fn test_list_experimental_features() {
        let registry = FeatureRegistry::new();
        let exp = registry.list_experimental_features();
        assert!(!exp.is_empty());
        assert!(
            exp.iter().any(
                |(k, name, _, _)| *k == FeatureKey::CodeMode && *name == "JavaScript Code Mode"
            )
        );
        assert!(
            exp.iter()
                .any(|(k, name, _, _)| *k == FeatureKey::RecapCatchupPrompt
                    && *name == "Context Recap Catchup Prompt")
        );
    }
}
