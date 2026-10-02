//! Unified Authoritative Configuration Control Plane (CFG-01..04).
//!
//! Provides the single authoritative `ResolvedConfiguration` used by the production runtime,
//! enforcing the 8-tier hierarchy:
//!   Tier 0: Immutable Security Invariants
//!   Tier 1: System Configuration (/etc/m31a/config.toml)
//!   Tier 2: User Configuration (~/.config/m31a/config.toml)
//!   Tier 3: Workspace Configuration (<workspace>/.m31a/config.toml)
//!   Tier 4: Profile (e.g. autonomous, coding, safe)
//!   Tier 5: Environment (M31A_*, std::env)
//!   Tier 6: CLI (--config, --model, --profile, --autonomy, etc.)
//!   Tier 7: Session Override (/model, /profile, runtime dynamic overrides)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::hierarchy::{ConfigPrecedenceEngine, ConfigTier};
use crate::config::merge::{ConfigError, MonotonicSecurityMerger, deep_merge_toml};
use crate::config::paths::PlatformPaths;
use crate::config::profile::ProfileResolver;
use crate::config::provenance::{ConfigLayer, ConfigurationService, ResolvedValue};
use crate::config::schema::{AppConfig, parse_and_validate_config, validate_config};

/// Diagnostic explanation of how a setting was resolved across tiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigExplain {
    pub key: String,
    pub resolved_value: serde_json::Value,
    pub winning_tier: String,
    pub winning_layer: ConfigLayer,
    pub source_file: Option<PathBuf>,
    pub is_immutable: bool,
    pub overrides: Vec<TierOverrideRecord>,
}

/// Record of an override at a specific tier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TierOverrideRecord {
    pub tier: ConfigTier,
    pub tier_name: String,
    pub value: serde_json::Value,
    pub source_file: Option<PathBuf>,
}

/// Authoritative resolved configuration for the M31A runtime.
/// Once constructed, this configuration is immutable, except for
/// explicitly supported session-scoped overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConfiguration {
    pub app_config: AppConfig,
    pub provenance: ConfigurationService,
    pub workspace_root: PathBuf,
    pub active_profile: Option<String>,
    pub active_model: String,
    pub active_provider: String,
    pub session_overrides: HashMap<String, serde_json::Value>,
    pub loaded_sources: Vec<(ConfigTier, PathBuf)>,
}

impl ResolvedConfiguration {
    /// Create a builder to resolve configuration for a workspace.
    pub fn builder(workspace_root: impl Into<PathBuf>) -> ResolvedConfigBuilder {
        ResolvedConfigBuilder::new(workspace_root)
    }

    /// Default resolved configuration for a workspace using standard safe defaults.
    pub fn for_workspace(workspace_root: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        Self::builder(workspace_root).build()
    }

    /// Fallback resolved configuration guaranteed never to fail.
    pub fn build_fallback(workspace_root: impl Into<PathBuf>) -> Self {
        Self::builder(workspace_root).build_fallback()
    }

    /// Retrieve the resolved provenance record for a specific key.
    pub fn get_resolved(&self, key: &str) -> Option<ResolvedValue> {
        self.provenance.resolve(key)
    }

    /// Explain a configuration key, detailing winning tier and all available tier values.
    pub fn explain(&self, key: &str) -> Option<ConfigExplain> {
        let resolved = self.provenance.resolve(key)?;

        // Mask secret if applicable
        let safe_value = if is_secret_key(key) {
            mask_value(&resolved.value)
        } else {
            resolved.value.clone()
        };

        let mut overrides = Vec::new();
        for (layer, val, src, _) in self.provenance.get_all_entries_for_key(key) {
            let safe_layer_val = if is_secret_key(key) {
                mask_value(&val)
            } else {
                val
            };
            overrides.push(TierOverrideRecord {
                tier: layer.into(),
                tier_name: layer.display_name().to_string(),
                value: safe_layer_val,
                source_file: src,
            });
        }

        Some(ConfigExplain {
            key: key.to_string(),
            resolved_value: safe_value,
            winning_tier: resolved.layer.display_name().to_string(),
            winning_layer: resolved.layer,
            source_file: resolved.source_file,
            is_immutable: resolved.is_immutable,
            overrides,
        })
    }

    /// Produce a diagnostic list of all loaded configuration sources.
    pub fn sources(&self) -> Vec<String> {
        let mut list = Vec::new();
        list.push("Tier 0: Built-in Safe Defaults (Kernel Binary)".to_string());
        for (tier, path) in &self.loaded_sources {
            list.push(format!("{}: {}", tier.name(), path.display()));
        }
        if let Some(ref prof) = self.active_profile {
            list.push(format!("Tier 4: Profile '{}'", prof));
        }
        if !self.session_overrides.is_empty() {
            list.push(format!(
                "Tier 7: {} Session Overrides Active",
                self.session_overrides.len()
            ));
        }
        list
    }

    /// Apply a session-scoped model override (/model command).
    ///
    /// Production session model switching is NVIDIA-only. A bare model ID
    /// (`meta/llama-3.1-70b-instruct`) or an explicit `nvidia/` qualifier
    /// resolves to the NVIDIA NIM provider. Any retired provider qualifier
    /// (`openai/`, `anthropic/`, `gemini/`, `local/`, `ollama/`,
    /// `openai_compatible/`) is rejected deterministically; the active
    /// provider is never silently switched to an unsupported provider.
    pub fn with_session_model(&self, model: &str) -> Result<Self, ConfigError> {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return Err(ConfigError::ValidationError(
                "Model name cannot be empty".to_string(),
            ));
        }

        let mut model_to_use = trimmed.to_string();

        if let Some((prov, model_part)) = trimmed.split_once('/') {
            let p_lower = prov.to_lowercase();
            if p_lower == "nvidia" || p_lower == "nvidia_nim" {
                model_to_use = model_part.to_string();
                if model_to_use.trim().is_empty() {
                    return Err(ConfigError::ValidationError(
                        "Model name cannot be empty".to_string(),
                    ));
                }
            } else if crate::config::provider_registry::is_retired_provider(&p_lower) {
                return Err(ConfigError::ValidationError(format!(
                    "Unsupported model provider '{prov}'. {}",
                    crate::config::provider_registry::NVIDIA_ONLY_ERROR
                )));
            } else if p_lower == "mock" {
                return Err(ConfigError::ValidationError(
                    "Mock provider is test-only and cannot be selected in normal interaction. Only NVIDIA NIM models are supported in this release."
                        .to_string(),
                ));
            }
            // Any other `publisher/model` form (e.g. `meta/llama-...`) is an
            // NVIDIA NIM hosted model ID: the full identifier is preserved
            // and the provider stays NVIDIA NIM.
        }

        // The session provider is always the production provider; a stale
        // configuration carrying a retired provider can never leak through a
        // model switch.
        let active_provider = crate::config::provider_registry::PRODUCTION_PROVIDER_ID.to_string();

        let mut updated = self.clone();
        updated.active_model = model_to_use.clone();
        updated.active_provider = active_provider.clone();
        updated.app_config.agents.default_model = model_to_use.clone();
        updated.app_config.provider.default = active_provider.clone();
        updated.session_overrides.insert(
            "agents.default_model".to_string(),
            serde_json::json!(model_to_use),
        );
        let _ = updated.provenance.set_value(
            ConfigLayer::Tier7Session,
            "agents.default_model",
            serde_json::json!(model_to_use),
            None,
            false,
        );
        let _ = updated.provenance.set_value(
            ConfigLayer::Tier7Session,
            "model",
            serde_json::json!(model_to_use),
            None,
            false,
        );
        Ok(updated)
    }

    /// Return the capability status of the currently active provider (WS-I §1, §10).
    pub fn active_provider_status(&self) -> crate::model::types::ProviderCapabilityStatus {
        let mut registry = crate::config::provider_registry::ProviderRegistry::new();
        let ws_creds = self.workspace_root.join(".m31a").join("credentials.json");
        if ws_creds.exists() {
            let _ = registry.load_credentials_from_file(&ws_creds);
        }
        registry.get_status(&self.active_provider)
    }

    /// Identify the configuration tier that supplied `provider.nvidia_nim.base_url`
    /// (P0-01 endpoint trust boundary).
    ///
    /// The winning provenance layer maps to an [`EndpointTrustSource`]:
    /// Tier0 → BuiltinDefault, Tier1 → System, Tier2 → User, Tier6 → ExplicitCli,
    /// Tier3 → Workspace (UNTRUSTED), Tier4 → SessionOverride-equivalent
    /// (untrusted for endpoint purposes), Tier5/Tier7 → SessionOverride.
    /// Unknown keys (default value, no explicit layer) → BuiltinDefault.
    pub fn provider_endpoint_source(&self) -> crate::model::provider::EndpointTrustSource {
        use crate::model::provider::EndpointTrustSource;
        let keys = [
            "provider.nvidia_nim.base_url",
            "provider.nvidia-nim.base_url",
        ];
        for key in keys {
            if let Some(explain) = self.explain(key) {
                return match explain.winning_layer {
                    ConfigLayer::Tier0SecurityInvariants => EndpointTrustSource::BuiltinDefault,
                    ConfigLayer::Tier1System => EndpointTrustSource::System,
                    ConfigLayer::Tier2User => EndpointTrustSource::User,
                    ConfigLayer::Tier6Cli => EndpointTrustSource::ExplicitCli,
                    ConfigLayer::Tier3Workspace => EndpointTrustSource::Workspace,
                    ConfigLayer::Tier4Profile
                    | ConfigLayer::Tier5Environment
                    | ConfigLayer::Tier7Session => EndpointTrustSource::SessionOverride,
                };
            }
        }
        EndpointTrustSource::BuiltinDefault
    }

    /// Apply a session-scoped profile override (/profile command).
    pub fn with_session_profile(&self, profile_name: &str) -> Result<Self, ConfigError> {
        let resolver = ProfileResolver::with_canonical_profiles();
        let mut profile_toml = resolver.resolve_profile(profile_name)?;

        // Ensure denied_tools from base are preserved in profile overlay
        if !self.app_config.policy.denied_tools.is_empty()
            && let toml::Value::Table(ref mut root_table) = profile_toml
        {
            let pol = root_table
                .entry("policy".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(pol_table) = pol {
                let existing = pol_table
                    .entry("denied_tools".to_string())
                    .or_insert_with(|| toml::Value::Array(Vec::new()));
                if let toml::Value::Array(arr) = existing {
                    let set: std::collections::HashSet<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    for d in &self.app_config.policy.denied_tools {
                        if !set.contains(d) {
                            arr.push(toml::Value::String(d.clone()));
                        }
                    }
                }
            }
        }

        // Check monotonic security: profile cannot weaken existing invariants
        let base_toml = toml::Value::try_from(&self.app_config)
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        MonotonicSecurityMerger::check(&base_toml, &profile_toml)?;

        let mut merged_toml = base_toml;
        let sanitized = sanitize_profile_for_app_config(&profile_toml);
        deep_merge_toml(&mut merged_toml, sanitized);

        let new_app_config: AppConfig = merged_toml
            .try_into()
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        validate_config(&new_app_config)?;

        let mut updated = self.clone();
        updated.app_config = new_app_config;
        updated.active_profile = Some(profile_name.to_string());
        updated.session_overrides.insert(
            "profile".to_string(),
            serde_json::Value::String(profile_name.to_string()),
        );
        let _ = updated.provenance.set_value(
            ConfigLayer::Tier7Session,
            "profile",
            serde_json::json!(profile_name),
            None,
            false,
        );
        Ok(updated)
    }

    /// Apply a session-scoped configuration override.
    pub fn with_session_override(
        &self,
        key: &str,
        value: serde_json::Value,
    ) -> Result<Self, ConfigError> {
        let mut updated = self.clone();
        let mut prov = updated.provenance.clone();
        prov.set_value(ConfigLayer::Tier7Session, key, value.clone(), None, false)
            .map_err(|e| ConfigError::SecurityDowngradeDenied {
                reason: e.to_string(),
            })?;

        let toml_val = json_to_toml(&value);
        let parts: Vec<&str> = key.split('.').collect();
        if !parts.is_empty() {
            let mut curr = toml_val;
            for part in parts.iter().rev() {
                let mut map = toml::map::Map::new();
                map.insert(part.to_string(), curr);
                curr = toml::Value::Table(map);
            }
            if let Ok(mut base_toml) = toml::Value::try_from(&updated.app_config) {
                MonotonicSecurityMerger::check(&base_toml, &curr)?;
                deep_merge_toml(&mut base_toml, curr);
                if let Ok(new_cfg) = base_toml.try_into() {
                    validate_config(&new_cfg)?;
                    updated.app_config = new_cfg;
                }
            }
        }

        if (key == "model" || key == "agents.default_model")
            && let serde_json::Value::String(ref m) = value
        {
            updated.active_model = m.clone();
        }

        updated.provenance = prov;
        updated.session_overrides.insert(key.to_string(), value);
        Ok(updated)
    }
}

/// Builder for constructing the authoritative ResolvedConfiguration.
pub struct ResolvedConfigBuilder {
    workspace_root: PathBuf,
    explicit_config_path: Option<PathBuf>,
    profile_override: Option<String>,
    autonomy_override: Option<String>,
    model_override: Option<String>,
    concurrency_override: Option<usize>,
    cli_layer: Option<toml::Value>,
    session_layer: Option<toml::Value>,
}

impl ResolvedConfigBuilder {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            explicit_config_path: None,
            profile_override: None,
            autonomy_override: None,
            model_override: None,
            concurrency_override: None,
            cli_layer: None,
            session_layer: None,
        }
    }

    pub fn with_explicit_config(mut self, path: Option<PathBuf>) -> Self {
        self.explicit_config_path = path;
        self
    }

    pub fn with_profile(mut self, profile: Option<String>) -> Self {
        self.profile_override = profile;
        self
    }

    pub fn with_autonomy(mut self, autonomy: Option<String>) -> Self {
        self.autonomy_override = autonomy;
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model_override = model;
        self
    }

    pub fn with_concurrency(mut self, limit: Option<usize>) -> Self {
        self.concurrency_override = limit;
        self
    }

    pub fn with_cli_layer(mut self, layer: toml::Value) -> Self {
        self.cli_layer = Some(layer);
        self
    }

    pub fn with_session_layer(mut self, layer: toml::Value) -> Self {
        self.session_layer = Some(layer);
        self
    }

    /// Build resolved configuration with automatic fallback to built-in safe defaults on error.
    pub fn build_fallback(self) -> ResolvedConfiguration {
        let ws = self.workspace_root.clone();
        self.build().unwrap_or_else(|_| ResolvedConfiguration {
            app_config: AppConfig::default(),
            provenance: ConfigurationService::new(),
            workspace_root: ws,
            active_profile: None,
            active_model: "meta/llama-3.2-11b-vision-instruct".to_string(),
            active_provider: "nvidia_nim".to_string(),
            session_overrides: HashMap::new(),
            loaded_sources: Vec::new(),
        })
    }

    /// Execute the complete hierarchical resolution across Tier 0 through Tier 7.
    pub fn build(self) -> Result<ResolvedConfiguration, ConfigError> {
        let mut engine = ConfigPrecedenceEngine::new();
        let mut provenance = ConfigurationService::new();
        let mut loaded_sources = Vec::new();
        let paths = PlatformPaths::new();

        // ---------------------------------------------------------------------
        // Tier 0: Built-in Safe Defaults and Security Invariants
        // ---------------------------------------------------------------------
        let default_config = AppConfig::default();
        let default_toml = toml::Value::try_from(&default_config)
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        engine.set_layer(ConfigTier::Tier0SecurityInvariants, default_toml.clone());
        record_provenance_table(
            &mut provenance,
            ConfigLayer::Tier0SecurityInvariants,
            &default_toml,
            None,
            false,
            "",
        );

        // Record immutable security invariants at Tier 0
        let _ = provenance.set_value(
            ConfigLayer::Tier0SecurityInvariants,
            "security.interactive_approvals_floor",
            serde_json::json!(true),
            None,
            true,
        );
        let _ = provenance.set_value(
            ConfigLayer::Tier0SecurityInvariants,
            "security.path_confinement",
            serde_json::json!(true),
            None,
            true,
        );
        let _ = provenance.set_value(
            ConfigLayer::Tier0SecurityInvariants,
            "security.credential_redaction",
            serde_json::json!(true),
            None,
            true,
        );
        let _ = provenance.set_value(
            ConfigLayer::Tier0SecurityInvariants,
            "security.mandatory_commit_trailers",
            serde_json::json!(true),
            None,
            true,
        );

        // ---------------------------------------------------------------------
        // Tier 1: System Configuration (/etc/m31a/config.toml)
        // ---------------------------------------------------------------------
        let sys_path = paths.system_config_file();
        if sys_path.is_file()
            && let Ok(content) = std::fs::read_to_string(&sys_path)
        {
            let validated_cfg = parse_and_validate_config(&content)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            let sys_toml = toml::Value::try_from(&validated_cfg)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            engine.set_layer(ConfigTier::Tier1System, sys_toml.clone());
            record_provenance_table(
                &mut provenance,
                ConfigLayer::Tier1System,
                &sys_toml,
                Some(sys_path.clone()),
                false,
                "",
            );
            loaded_sources.push((ConfigTier::Tier1System, sys_path));
        }

        // ---------------------------------------------------------------------
        // Tier 2: User Configuration (~/.config/m31a/config.toml)
        // ---------------------------------------------------------------------
        let user_path = paths.user_config_file();
        if user_path.is_file()
            && let Ok(content) = std::fs::read_to_string(&user_path)
        {
            let validated_cfg = parse_and_validate_config(&content)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            let user_toml = toml::Value::try_from(&validated_cfg)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            engine.set_layer(ConfigTier::Tier2User, user_toml.clone());
            record_provenance_table(
                &mut provenance,
                ConfigLayer::Tier2User,
                &user_toml,
                Some(user_path.clone()),
                false,
                "",
            );
            loaded_sources.push((ConfigTier::Tier2User, user_path));
        }

        // ---------------------------------------------------------------------
        // Tier 3: Workspace Configuration (<ws>/.m31a/config.toml)
        // ---------------------------------------------------------------------
        let ws_path = PlatformPaths::workspace_config_file(&self.workspace_root);
        if ws_path.is_file()
            && let Ok(content) = std::fs::read_to_string(&ws_path)
        {
            let validated_cfg = parse_and_validate_config(&content)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            let ws_toml = toml::Value::try_from(&validated_cfg)
                .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            engine.set_layer(ConfigTier::Tier3Workspace, ws_toml.clone());
            record_provenance_table(
                &mut provenance,
                ConfigLayer::Tier3Workspace,
                &ws_toml,
                Some(ws_path.clone()),
                false,
                "",
            );
            loaded_sources.push((ConfigTier::Tier3Workspace, ws_path));
        }

        // ---------------------------------------------------------------------
        // Explicit --config <path> (overrides Workspace / User file settings)
        // ---------------------------------------------------------------------
        if let Some(ref explicit_path) = self.explicit_config_path {
            if explicit_path.is_file() {
                let content = std::fs::read_to_string(explicit_path)
                    .map_err(|e| ConfigError::IoError(e.to_string()))?;
                let explicit_cfg = parse_and_validate_config(&content)
                    .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
                let explicit_toml = toml::Value::try_from(&explicit_cfg)
                    .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
                // Place on Workspace tier (or overlay on CLI tier if already present)
                engine.set_layer(ConfigTier::Tier3Workspace, explicit_toml.clone());
                record_provenance_table(
                    &mut provenance,
                    ConfigLayer::Tier6Cli,
                    &explicit_toml,
                    Some(explicit_path.clone()),
                    false,
                    "",
                );
                loaded_sources.push((ConfigTier::Tier6Cli, explicit_path.clone()));
            } else {
                return Err(ConfigError::IoError(format!(
                    "Explicit config file not found: {}",
                    explicit_path.display()
                )));
            }
        }

        // ---------------------------------------------------------------------
        // Tier 4: Profile
        // ---------------------------------------------------------------------
        let profile_resolver = ProfileResolver::with_canonical_profiles();
        let active_profile = self.profile_override.clone();
        if let Some(ref prof_name) = active_profile {
            let prof_toml = profile_resolver.resolve_profile(prof_name)?;
            let sanitized_prof = sanitize_profile_for_app_config(&prof_toml);
            engine.set_layer(ConfigTier::Tier4Profile, sanitized_prof);
            record_provenance_table(
                &mut provenance,
                ConfigLayer::Tier4Profile,
                &prof_toml,
                None,
                false,
                "",
            );
            let _ = provenance.set_value(
                ConfigLayer::Tier4Profile,
                "profile",
                serde_json::json!(prof_name),
                None,
                false,
            );
        }

        // ---------------------------------------------------------------------
        // Tier 5: Environment Overrides
        // ---------------------------------------------------------------------
        let mut env_table = toml::map::Map::new();
        if let Ok(model_env) =
            std::env::var("M31A_MODEL").or_else(|_| std::env::var("NVIDIA_MODEL"))
            && !model_env.trim().is_empty()
        {
            let mut agents_map = toml::map::Map::new();
            agents_map.insert(
                "default_model".to_string(),
                toml::Value::String(model_env.trim().to_string()),
            );
            env_table.insert("agents".to_string(), toml::Value::Table(agents_map));
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "agents.default_model",
                serde_json::json!(model_env.trim()),
                None,
                false,
            );
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "model",
                serde_json::json!(model_env.trim()),
                None,
                false,
            );
        }

        if let Ok(conc_env) = std::env::var("M31A_CONCURRENCY")
            && let Ok(limit) = conc_env.trim().parse::<i64>()
        {
            let rt_val = env_table
                .entry("runtime".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(rt_map) = rt_val {
                rt_map.insert("concurrency_limit".to_string(), toml::Value::Integer(limit));
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "runtime.concurrency_limit",
                serde_json::json!(limit),
                None,
                false,
            );
        }

        if let Ok(timeout_env) =
            std::env::var("M31A_TIMEOUT").or_else(|_| std::env::var("M31A_TIMEOUT_SECS"))
            && let Ok(secs) = timeout_env.trim().parse::<i64>()
        {
            let rt_val = env_table
                .entry("runtime".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(rt_map) = rt_val {
                rt_map.insert("timeout_secs".to_string(), toml::Value::Integer(secs));
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "runtime.timeout_secs",
                serde_json::json!(secs),
                None,
                false,
            );
        }

        if let Ok(theme_env) = std::env::var("M31A_THEME")
            && !theme_env.trim().is_empty()
        {
            let mut tui_map = toml::map::Map::new();
            tui_map.insert(
                "theme".to_string(),
                toml::Value::String(theme_env.trim().to_string()),
            );
            env_table.insert("tui".to_string(), toml::Value::Table(tui_map));
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "tui.theme",
                serde_json::json!(theme_env.trim()),
                None,
                false,
            );
        }

        if let Ok(steps_env) = std::env::var("M31A_MAX_STEPS")
            .or_else(|_| std::env::var("M31A_MAX_AGENT_STEPS"))
            .or_else(|_| std::env::var("M31A_BUDGET_MAX_AGENT_STEPS"))
            && let Ok(steps) = steps_env.trim().parse::<i64>()
        {
            let b_val = env_table
                .entry("budget".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(b_map) = b_val {
                b_map.insert("max_agent_steps".to_string(), toml::Value::Integer(steps));
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "budget.max_agent_steps",
                serde_json::json!(steps),
                None,
                false,
            );
        }

        if let Ok(commit_env) =
            std::env::var("M31A_AUTO_COMMIT").or_else(|_| std::env::var("M31A_GIT_AUTO_COMMIT"))
            && let Ok(b) = commit_env.trim().parse::<bool>()
        {
            let g_val = env_table
                .entry("git".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(g_map) = g_val {
                g_map.insert("auto_commit".to_string(), toml::Value::Boolean(b));
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "git.auto_commit",
                serde_json::json!(b),
                None,
                false,
            );
        }

        if let Ok(test_env) = std::env::var("M31A_TEST_COMMAND")
            .or_else(|_| std::env::var("M31A_VERIFICATION_TEST_COMMAND"))
            && !test_env.trim().is_empty()
        {
            let ws_val = env_table
                .entry("workspace".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(ws_map) = ws_val {
                let ver_val = ws_map
                    .entry("verification".to_string())
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let toml::Value::Table(ver_map) = ver_val {
                    ver_map.insert(
                        "test_command".to_string(),
                        toml::Value::String(test_env.trim().to_string()),
                    );
                }
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "workspace.verification.test_command",
                serde_json::json!(test_env.trim()),
                None,
                false,
            );
        }

        if let Ok(denied_env) = std::env::var("M31A_DENIED_TOOLS")
            && !denied_env.trim().is_empty()
        {
            let list: Vec<toml::Value> = denied_env
                .split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect();
            let json_list: Vec<serde_json::Value> = denied_env
                .split(',')
                .map(|s| serde_json::Value::String(s.trim().to_string()))
                .collect();
            let pol_val = env_table
                .entry("policy".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let toml::Value::Table(pol_map) = pol_val {
                pol_map.insert("denied_tools".to_string(), toml::Value::Array(list));
            }
            let _ = provenance.set_value(
                ConfigLayer::Tier5Environment,
                "policy.denied_tools",
                serde_json::Value::Array(json_list),
                None,
                false,
            );
        }

        if !env_table.is_empty() {
            let env_toml = toml::Value::Table(env_table);
            engine.set_layer(ConfigTier::Tier5Environment, env_toml);
        }

        // ---------------------------------------------------------------------
        // Tier 6: CLI Overrides
        // ---------------------------------------------------------------------
        let mut cli_table = toml::map::Map::new();

        if let Some(ref m) = self.model_override {
            let mut agents_map = toml::map::Map::new();
            agents_map.insert("default_model".to_string(), toml::Value::String(m.clone()));
            cli_table.insert("agents".to_string(), toml::Value::Table(agents_map));
            let _ = provenance.set_value(
                ConfigLayer::Tier6Cli,
                "agents.default_model",
                serde_json::json!(m),
                None,
                false,
            );
            let _ = provenance.set_value(
                ConfigLayer::Tier6Cli,
                "model",
                serde_json::json!(m),
                None,
                false,
            );
        }

        if let Some(ref auto) = self.autonomy_override {
            cli_table.insert(
                "autonomy_mode".to_string(),
                toml::Value::String(auto.clone()),
            );
            let _ = provenance.set_value(
                ConfigLayer::Tier6Cli,
                "autonomy_mode",
                serde_json::json!(auto),
                None,
                false,
            );
        }

        if let Some(limit) = self.concurrency_override {
            let mut rt_map = toml::map::Map::new();
            rt_map.insert(
                "concurrency_limit".to_string(),
                toml::Value::Integer(limit as i64),
            );
            cli_table.insert("runtime".to_string(), toml::Value::Table(rt_map));
            let _ = provenance.set_value(
                ConfigLayer::Tier6Cli,
                "runtime.concurrency_limit",
                serde_json::json!(limit),
                None,
                false,
            );
        }

        if let Some(toml::Value::Table(ref extra_table)) = self.cli_layer {
            for (k, v) in extra_table {
                cli_table.insert(k.clone(), v.clone());
            }
        }

        if !cli_table.is_empty() {
            let cli_toml = toml::Value::Table(cli_table);
            engine.set_layer(ConfigTier::Tier6Cli, cli_toml);
        }

        // ---------------------------------------------------------------------
        // Tier 7: Session Layer (if supplied)
        // ---------------------------------------------------------------------
        if let Some(ref sess_toml) = self.session_layer {
            engine.set_layer(ConfigTier::Tier7Session, sess_toml.clone());
            record_provenance_table(
                &mut provenance,
                ConfigLayer::Tier7Session,
                sess_toml,
                None,
                false,
                "",
            );
        }

        // ---------------------------------------------------------------------
        // Precedence Resolution & Monotonic Security Validation
        // ---------------------------------------------------------------------
        let resolved_toml = engine.resolve_raw()?;
        let app_config: AppConfig = resolved_toml
            .try_into()
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        validate_config(&app_config)?;

        let active_model = app_config.agents.default_model.clone();
        // Canonicalize the production provider ID (`nvidia` alias folds to
        // `nvidia_nim`). Schema validation above already guarantees the value
        // normalizes to NVIDIA NIM; this keeps every downstream consumer on
        // the canonical spelling.
        let active_provider = crate::config::provider_registry::normalize_provider_id(
            app_config.provider.default.trim(),
        );

        Ok(ResolvedConfiguration {
            app_config,
            provenance,
            workspace_root: self.workspace_root,
            active_profile,
            active_model,
            active_provider,
            session_overrides: HashMap::new(),
            loaded_sources,
        })
    }
}

/// Recursively record table keys in ConfigurationService with hierarchical names.
fn record_provenance_table(
    service: &mut ConfigurationService,
    layer: ConfigLayer,
    toml_val: &toml::Value,
    source_file: Option<PathBuf>,
    is_immutable: bool,
    prefix: &str,
) {
    if let toml::Value::Table(table) = toml_val {
        for (k, v) in table {
            let full_key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{}.{}", prefix, k)
            };

            match v {
                toml::Value::Table(_) => {
                    record_provenance_table(
                        service,
                        layer,
                        v,
                        source_file.clone(),
                        is_immutable,
                        &full_key,
                    );
                }
                _ => {
                    let json_val = serde_json::to_value(v).unwrap_or_default();
                    let _ = service.set_value(
                        layer,
                        &full_key,
                        json_val,
                        source_file.clone(),
                        is_immutable,
                    );
                }
            }
        }
    }
}

/// Check if a configuration key represents a credential or secret.
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("api_key")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.ends_with("_key")
        || lower.contains("credential")
        || lower.contains("header")
        || lower.contains("auth")
}

/// Mask sensitive string values.
pub fn mask_value(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => {
            if s.is_empty() {
                serde_json::Value::String(String::new())
            } else if s.len() <= 6 {
                serde_json::Value::String("********".to_string())
            } else {
                let prefix = &s[..std::cmp::min(4, s.len())];
                serde_json::Value::String(format!("{}...[MASKED]", prefix))
            }
        }
        _ => val.clone(),
    }
}

/// Strip non-AppConfig metadata fields from profile table (e.g. name, capabilities, verification_tier).
fn sanitize_profile_for_app_config(prof_toml: &toml::Value) -> toml::Value {
    let mut sanitized = toml::map::Map::new();
    if let toml::Value::Table(tbl) = prof_toml {
        for (k, v) in tbl {
            if matches!(
                k.as_str(),
                "runtime"
                    | "policy"
                    | "agents"
                    | "budget"
                    | "tui"
                    | "provider"
                    | "workspace"
                    | "git"
                    | "plugins"
                    | "profiles"
            ) {
                sanitized.insert(k.clone(), v.clone());
            }
        }
    }
    toml::Value::Table(sanitized)
}

/// Convert a serde_json::Value directly to an equivalent toml::Value.
pub fn json_to_toml(val: &serde_json::Value) -> toml::Value {
    match val {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(b) => toml::Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => toml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => toml::Value::Array(arr.iter().map(json_to_toml).collect()),
        serde_json::Value::Object(map) => {
            let mut tmap = toml::map::Map::new();
            for (k, v) in map {
                tmap.insert(k.clone(), json_to_toml(v));
            }
            toml::Value::Table(tmap)
        }
    }
}
