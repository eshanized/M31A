//! Central M31A configuration loader, schema validator, and profile merger.
//!
//! Loads hierarchical configuration files (`~/.config/m31a/config.toml`, `.m31a/config.toml`, CLI flags),
//! resolving API keys, sandbox policies, approval modes, and model settings.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default)]
    pub review_model: Option<String>,

    #[serde(default)]
    pub model_provider: Option<String>,

    #[serde(default = "default_context_window")]
    pub model_context_window: usize,

    #[serde(default = "default_compact_limit")]
    pub model_auto_compact_token_limit: usize,

    #[serde(default = "default_post_turn_compact_pct")]
    pub model_post_turn_compact_threshold_percent: u8,

    #[serde(default = "default_api_base")]
    pub api_base: String,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default = "default_sandbox")]
    pub sandbox: SandboxPolicy,

    #[serde(default = "default_approval")]
    pub approval: ApprovalPolicy,

    #[serde(default = "default_approvals_reviewer")]
    pub approvals_reviewer: String,

    #[serde(default)]
    pub auto_review: Option<AutoReviewConfig>,

    #[serde(default = "default_true")]
    pub allow_login_shell: bool,

    #[serde(default = "default_project_doc_max_bytes")]
    pub project_doc_max_bytes: usize,

    #[serde(default = "default_project_doc_fallback_filenames")]
    pub project_doc_fallback_filenames: Vec<String>,

    #[serde(default = "default_tool_output_token_limit")]
    pub tool_output_token_limit: usize,

    #[serde(default = "default_web_search")]
    pub web_search: String,

    #[serde(default)]
    pub profile: Option<String>,

    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,

    #[serde(default)]
    pub tui: TuiConfig,

    #[serde(default)]
    pub notify: Option<Vec<String>>,

    #[serde(default)]
    pub memories: MemoriesConfig,

    #[serde(default)]
    pub hooks: HooksConfig,

    #[serde(default)]
    pub skills: SkillsConfig,

    #[serde(default)]
    pub agents: AgentsConfig,

    #[serde(default)]
    pub goals: GoalsConfig,

    #[serde(default = "default_features")]
    pub features: HashMap<String, bool>,

    #[serde(default)]
    pub extra_dirs: Vec<PathBuf>,

    #[serde(default = "default_true")]
    pub auto_start_daemon: bool,

    /// Opt-in dynamic model routing per task tier (`HybridModelRouter`).
    /// Disabled by default so the configured `model` remains authoritative.
    #[serde(default)]
    pub routing_enabled: bool,

    /// Reasoning effort override for thinking/reasoning models ("low", "medium", "high").
    #[serde(default)]
    pub reasoning_effort: Option<String>,

    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    #[serde(default)]
    pub mcp: McpConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Whether to prefix MCP tool names with "mcp__" (default: true).
    /// When false, tool names will be "server__tool" format.
    #[serde(default = "default_true")]
    pub prefix_tool_names: bool,

    /// Maximum tool name length before truncation (default: 128).
    #[serde(default = "default_mcp_max_name_length")]
    pub max_tool_name_length: usize,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            prefix_tool_names: true,
            max_tool_name_length: 128,
        }
    }
}

fn default_mcp_max_name_length() -> usize {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoReviewConfig {
    #[serde(default)]
    pub timeout_ms: Option<u32>,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub circuit_break_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    #[serde(default = "default_true")]
    pub animations: bool,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub icons: Option<String>,
    #[serde(default)]
    pub vim_mode_default: bool,
    #[serde(default = "default_true", alias = "mouse")]
    pub mouse_enabled: bool,
    #[serde(default = "default_true")]
    pub scroll_acceleration: bool,
    #[serde(default = "default_scroll_speed")]
    pub mouse_scroll_speed: f64,
    #[serde(default = "default_true")]
    pub copy_on_select: bool,
    #[serde(default = "default_status_line")]
    pub status_line: Vec<String>,
    #[serde(default = "default_terminal_title")]
    pub terminal_title: Vec<String>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            animations: true,
            theme: None,
            icons: None,
            vim_mode_default: false,
            mouse_enabled: true,
            scroll_acceleration: true,
            mouse_scroll_speed: 1.0,
            copy_on_select: true,
            status_line: default_status_line(),
            terminal_title: default_terminal_title(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoriesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub extract_rollouts: bool,
    #[serde(default = "default_consolidation_interval")]
    pub consolidation_interval_turns: usize,
}

impl Default for MemoriesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extract_rollouts: true,
            consolidation_interval_turns: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub bypass_hook_trust: bool,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bypass_hook_trust: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Configurable multi-agent resource governance policies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceGovernance {
    /// Maximum concurrently running child agents across the entire runtime.
    #[serde(default = "default_agent_max_threads")]
    pub max_concurrent_threads: usize,
    /// Maximum spawn-tree depth (root = 0, child = 1, grandchild = 2, etc.).
    #[serde(default = "default_agent_max_depth")]
    pub max_spawn_depth: usize,
    /// Maximum turns a child agent is permitted to execute per run_turn.
    #[serde(default = "default_agent_max_child_turns")]
    pub max_child_turns: usize,
    /// Optional aggregate token budget enforced atomically across the entire agent hierarchy.
    #[serde(default)]
    pub max_aggregate_tokens: Option<usize>,
}

pub fn default_agent_max_child_turns() -> usize {
    20
}

impl Default for ResourceGovernance {
    fn default() -> Self {
        Self {
            max_concurrent_threads: default_agent_max_threads(),
            max_spawn_depth: default_agent_max_depth(),
            max_child_turns: default_agent_max_child_turns(),
            max_aggregate_tokens: None,
        }
    }
}

impl ResourceGovernance {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_concurrent_threads == 0 {
            return Err("ResourceGovernance: max_concurrent_threads must be >= 1".to_string());
        }
        if self.max_spawn_depth == 0 {
            return Err("ResourceGovernance: max_spawn_depth must be >= 1".to_string());
        }
        if self.max_child_turns == 0 {
            return Err("ResourceGovernance: max_child_turns must be >= 1".to_string());
        }
        if self.max_aggregate_tokens == Some(0) {
            return Err(
                "ResourceGovernance: max_aggregate_tokens must be > 0 if specified".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default = "default_agent_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_agent_max_threads")]
    pub max_threads: usize,
    #[serde(default = "default_agent_max_child_turns")]
    pub max_child_turns: usize,
    #[serde(default)]
    pub governance: Option<ResourceGovernance>,
}

impl AgentsConfig {
    pub fn effective_governance(&self) -> ResourceGovernance {
        if let Some(ref gov) = self.governance {
            gov.clone()
        } else {
            ResourceGovernance {
                max_concurrent_threads: self.max_threads,
                max_spawn_depth: self.max_depth,
                max_child_turns: self.max_child_turns,
                max_aggregate_tokens: None,
            }
        }
    }
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            max_depth: default_agent_max_depth(),
            max_threads: default_agent_max_threads(),
            max_child_turns: default_agent_max_child_turns(),
            governance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigProfile {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub sandbox: Option<SandboxPolicy>,
    #[serde(default)]
    pub approval: Option<ApprovalPolicy>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
    #[serde(default)]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServerConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub transport: Option<String>, // "stdio", "sse", "websocket"
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(default)]
    pub require_oauth: Option<bool>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl McpServerConfig {
    pub fn new(
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self {
            command: command.into(),
            args,
            env,
            url: None,
            transport: None,
            headers: None,
            allowed_domains: None,
            blocked_domains: None,
            require_oauth: None,
            timeout_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxPolicy {
    ReadOnly,
    #[default]
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    #[default]
    OnRequest,
    Never,
    ApproveForMe, // Guardian review
}

fn default_model() -> String {
    "nvidia/nemotron-3.5-lightning-30b-a3b".to_string()
}

fn default_context_window() -> usize {
    128_000
}

fn default_compact_limit() -> usize {
    100_000
}

fn default_post_turn_compact_pct() -> u8 {
    80
}

fn default_api_base() -> String {
    "https://integrate.api.nvidia.com/v1".to_string()
}

fn default_sandbox() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite
}

fn default_approval() -> ApprovalPolicy {
    ApprovalPolicy::OnRequest
}

fn default_approvals_reviewer() -> String {
    "user".to_string()
}

fn default_project_doc_max_bytes() -> usize {
    32 * 1024
}

fn default_project_doc_fallback_filenames() -> Vec<String> {
    vec![
        "AGENTS.md".to_string(),
        "CLAUDE.md".to_string(),
        "README.md".to_string(),
    ]
}

fn default_tool_output_token_limit() -> usize {
    16 * 1024
}

fn default_web_search() -> String {
    "live".to_string()
}

fn default_scroll_speed() -> f64 {
    1.0
}

fn default_status_line() -> Vec<String> {
    vec![
        "model-with-reasoning".to_string(),
        "current-dir".to_string(),
        "thread-name".to_string(),
    ]
}

fn default_terminal_title() -> Vec<String> {
    vec![
        "activity".to_string(),
        "thread-name".to_string(),
        "project-name".to_string(),
    ]
}

fn default_consolidation_interval() -> usize {
    5
}

fn default_agent_max_depth() -> usize {
    3
}

fn default_agent_max_threads() -> usize {
    16
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: default_model(),
            review_model: None,
            model_provider: Some("nvidia".to_string()),
            model_context_window: default_context_window(),
            model_auto_compact_token_limit: default_compact_limit(),
            model_post_turn_compact_threshold_percent: default_post_turn_compact_pct(),
            api_base: default_api_base(),
            api_key: resolve_stored_api_key(),
            sandbox: default_sandbox(),
            approval: default_approval(),
            approvals_reviewer: default_approvals_reviewer(),
            auto_review: None,
            allow_login_shell: true,
            project_doc_max_bytes: default_project_doc_max_bytes(),
            project_doc_fallback_filenames: default_project_doc_fallback_filenames(),
            tool_output_token_limit: default_tool_output_token_limit(),
            web_search: default_web_search(),
            profile: None,
            profiles: HashMap::new(),
            tui: TuiConfig::default(),
            notify: None,
            memories: MemoriesConfig::default(),
            hooks: HooksConfig::default(),
            skills: SkillsConfig::default(),
            agents: AgentsConfig::default(),
            goals: GoalsConfig::default(),
            features: default_features(),
            extra_dirs: Vec::new(),
            auto_start_daemon: true,
            routing_enabled: false,
            reasoning_effort: None,
            mcp_servers: HashMap::new(),
            mcp: McpConfig::default(),
        }
    }
}

pub fn default_features() -> HashMap<String, bool> {
    let mut m = HashMap::new();
    // Only real `FeatureKey` names belong here; legacy placeholder keys
    // (guardian_approval, bwrap_sandbox, streaming, ...) were removed — they
    // were never resolvable by `FeatureKey::from_str_name` and silently did nothing.
    m.insert("code_mode".to_string(), true);
    m.insert("guardian_v2".to_string(), true);
    m.insert("apply_patch_streaming".to_string(), true);
    m.insert("unified_exec".to_string(), true);
    m.insert("write_stdin_approval".to_string(), true);
    m.insert("exec_permission_approvals".to_string(), true);
    m.insert("code_mode_only".to_string(), false);
    m.insert("code_mode_only_strict_3p_tools".to_string(), false);
    m.insert("code_mode_tool_search".to_string(), false);
    m.insert("standalone_web_search".to_string(), false);
    m.insert("deferred_executor".to_string(), false);
    m.insert("token_budget".to_string(), true);
    m.insert("current_time_reminder".to_string(), true);
    m.insert("multi_agent_v2".to_string(), true);
    m.insert("agent_message_board".to_string(), true);
    m.insert("image_generation".to_string(), true);
    m.insert("unified_image_budget".to_string(), false);
    m.insert("apply_patch_streaming_events".to_string(), true);
    m.insert("guardian_native_multi_agent_history".to_string(), false);
    m.insert("deferred_tool_world_state".to_string(), true);
    m.insert("non_prefixed_mcp_tool_names".to_string(), false);
    m.insert("worktree_management".to_string(), true);
    m.insert("voice_input".to_string(), false);
    m.insert("web_search".to_string(), true);
    m
}

impl Config {
    /// Returns a sanitized public JSON representation of the configuration.
    /// Redacts raw API keys, OAuth tokens, and sensitive MCP environment variables and headers,
    /// while preserving presence indicators.
    pub fn sanitized_value(&self) -> serde_json::Value {
        let mut val = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        if let Some(obj) = val.as_object_mut() {
            let has_key = self
                .api_key
                .as_ref()
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false);
            obj.insert("api_key_configured".to_string(), serde_json::json!(has_key));
            obj.insert(
                "api_key".to_string(),
                if has_key {
                    serde_json::json!("[REDACTED]")
                } else {
                    serde_json::Value::Null
                },
            );

            // Sanitize mcp_servers
            if let Some(mcp) = obj.get_mut("mcp_servers").and_then(|v| v.as_object_mut()) {
                for (_name, server_val) in mcp.iter_mut() {
                    if let Some(srv_obj) = server_val.as_object_mut() {
                        if let Some(env_obj) =
                            srv_obj.get_mut("env").and_then(|v| v.as_object_mut())
                        {
                            for (_env_k, env_v) in env_obj.iter_mut() {
                                *env_v = serde_json::json!("[REDACTED]");
                            }
                        }
                        if let Some(hdr_obj) =
                            srv_obj.get_mut("headers").and_then(|v| v.as_object_mut())
                        {
                            for (_hdr_k, hdr_v) in hdr_obj.iter_mut() {
                                *hdr_v = serde_json::json!("[REDACTED]");
                            }
                        }
                    }
                }
            }

            // Sanitize profiles
            if let Some(profiles) = obj.get_mut("profiles").and_then(|v| v.as_object_mut()) {
                for pval in profiles.values_mut() {
                    if let Some(pobj) = pval.as_object_mut() {
                        pobj.entry("api_key").and_modify(|k| {
                            if !k.is_null() {
                                *k = serde_json::json!("[REDACTED]");
                            }
                        });
                    }
                }
            }
        }
        val
    }

    pub fn config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_HOME") {
            PathBuf::from(dir)
        } else if let Some(proj) = ProjectDirs::from("org", "m31a", "m31a") {
            proj.config_dir().to_path_buf()
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".m31a")
        }
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        Self::load_with_overrides(None, &[], false)
    }

    /// Loads config with full 10-layer layering (report §7.2, upstream
    /// `config/src/loader/mod.rs:123-136`):
    ///  1. Embedded defaults (Config::default())
    ///  2. Admin/MDM config (if M31A_ADMIN_CONFIG set)
    ///  3. System config `/etc/m31a/config.toml`
    ///  4. Cloud bundle fragments (if M31A_CLOUD_BUNDLE_DIR set)
    ///  5. User config `~/.m31a/config.toml`
    ///  6. Profile config `~/.m31a/<profile>.config.toml` or inline `[profile.<name>]`
    ///  7. Repo config `.m31a/config.toml` (skipped when untrusted)
    ///  8. Environment variable overrides
    ///  9. Runtime CLI overrides (`-c`)
    /// 10. Legacy managed config (if any)
    ///
    /// `strict` enables `--strict-config` semantics: unknown fields escalate
    /// to errors (schemars `deny_unknown_fields`).
    pub fn load_with_overrides(
        profile_name: Option<&str>,
        overrides: &[String],
        strict: bool,
    ) -> Result<Self> {
        let mut config = Config::default();

        // Layer 2: Admin/MDM config (highest priority after defaults)
        if let Ok(admin_path) = std::env::var("M31A_ADMIN_CONFIG")
            && let Ok(text) = fs::read_to_string(&admin_path)
        {
            let loaded = Self::load_layer(&text, strict)
                .with_context(|| format!("Failed loading admin config: {}", admin_path))?;
            config = Self::merge_config(config, loaded);
        }

        // Layer 3: System config
        let system_path = Path::new("/etc/m31a/config.toml");
        if system_path.exists()
            && let Ok(text) = fs::read_to_string(system_path)
        {
            let loaded = Self::load_layer(&text, strict)
                .with_context(|| "Failed loading system config /etc/m31a/config.toml")?;
            config = Self::merge_config(config, loaded);
        }

        // Layer 4: Cloud bundle fragments
        if let Ok(cloud_dir) = std::env::var("M31A_CLOUD_BUNDLE_DIR") {
            let cloud_path = Path::new(&cloud_dir).join("config.toml");
            if cloud_path.exists()
                && let Ok(text) = fs::read_to_string(&cloud_path)
            {
                let loaded = Self::load_layer(&text, strict).with_context(|| {
                    format!("Failed loading cloud bundle: {}", cloud_path.display())
                })?;
                config = Self::merge_config(config, loaded);
            }
        }

        // Layer 5: User config
        let user_path = Self::config_path();
        if user_path.exists()
            && let Ok(text) = fs::read_to_string(&user_path)
        {
            let loaded = Self::load_layer(&text, strict)
                .with_context(|| format!("Failed loading user config: {}", user_path.display()))?;
            config = Self::merge_config(config, loaded);
        }

        // Layer 6: Profile config (file or inline)
        let selected_profile = profile_name
            .map(|s| s.to_string())
            .or_else(|| config.profile.clone())
            .or_else(|| std::env::var("M31A_PROFILE").ok());

        if let Some(ref prof) = selected_profile {
            // Try external profile file first
            let prof_path = Self::config_dir().join(format!("{}.config.toml", prof));
            if prof_path.exists() {
                if let Ok(text) = fs::read_to_string(&prof_path) {
                    let loaded =
                        Self::load_layer::<ConfigProfile>(&text, strict).with_context(|| {
                            format!("Failed loading profile config: {}", prof_path.display())
                        })?;
                    config = Self::merge_profile(config, loaded);
                }
            } else {
                // Clone the profile config to avoid borrow conflict
                if let Some(prof_cfg) = config.profiles.get(prof).cloned() {
                    config = Self::merge_profile(config, prof_cfg);
                }
            }
        }

        // Layer 7: Repo config (only if trusted)
        if let Some(proj_cfg) = find_project_config() {
            // Check trust: repo configs are skipped when untrusted (upstream semantics)
            let trusted = Self::is_repo_trusted(&proj_cfg).unwrap_or(false);
            if trusted {
                if let Ok(text) = fs::read_to_string(&proj_cfg) {
                    let loaded = Self::load_layer(&text, strict).with_context(|| {
                        format!("Failed loading repo config: {}", proj_cfg.display())
                    })?;
                    config = Self::merge_repo_config(config, loaded);
                }
            } else {
                tracing::warn!("Skipping untrusted repo config at {}", proj_cfg.display());
            }
        }

        // Layer 8: Environment variable overrides
        if let Ok(key) =
            std::env::var("NVIDIA_API_KEY").or_else(|_| std::env::var("NVIDIA_NIM_API_KEY"))
        {
            config.api_key = Some(key);
        }
        if let Ok(base) = std::env::var("NVIDIA_API_BASE") {
            config.api_base = base;
        }
        if let Ok(m) = std::env::var("M31A_MODEL") {
            config.model = m;
        }
        if let Ok(s) = std::env::var("M31A_SANDBOX") {
            let _ = config.apply_override("sandbox", &s);
        }
        if let Ok(a) = std::env::var("M31A_APPROVAL") {
            let _ = config.apply_override("approval", &a);
        }

        // Layer 9: Runtime CLI overrides
        for item in overrides {
            if let Some((k, v)) = item.split_once('=') {
                let _ = config.apply_override(k.trim(), v.trim());
            }
        }

        // Layer 10: Legacy managed config (if any) — placeholder for future migration
        // config = apply_legacy_managed_config(config);

        Ok(config)
    }

    /// Helper to deserialize a config layer with optional strict unknown-field rejection.
    fn load_layer<T: DeserializeOwned>(text: &str, strict: bool) -> Result<T> {
        let loaded = toml::from_str(text).with_context(|| "TOML parse failed")?;
        if strict {
            let table: toml::Table = toml::from_str(text)
                .with_context(|| "Failed parsing TOML table for strict validation")?;
            let allowed_keys: std::collections::HashSet<&str> = [
                "model",
                "review_model",
                "model_provider",
                "model_context_window",
                "model_auto_compact_token_limit",
                "model_post_turn_compact_threshold_percent",
                "api_base",
                "api_key",
                "sandbox",
                "approval",
                "approvals_reviewer",
                "auto_review",
                "allow_login_shell",
                "project_doc_max_bytes",
                "project_doc_fallback_filenames",
                "tool_output_token_limit",
                "web_search",
                "profile",
                "profiles",
                "tui",
                "notify",
                "memories",
                "hooks",
                "skills",
                "agents",
                "goals",
                "features",
                "extra_dirs",
                "auto_start_daemon",
                "routing_enabled",
                "reasoning_effort",
                "mcp_servers",
                "mcp",
                "temperature",
                "top_p",
                "max_tokens",
                "system_prompt",
                "proxy",
                "guardian",
            ]
            .into_iter()
            .collect();

            for (key, _) in &table {
                if !allowed_keys.contains(key.as_str()) {
                    anyhow::bail!(
                        "Strict config validation error: unknown top-level configuration key '{}'",
                        key
                    );
                }
            }

            if let Ok(json_val) = serde_json::to_value(&table) {
                let structural_warnings =
                    crate::config_schema::validate_config_structure(&json_val);
                if !structural_warnings.is_empty() {
                    anyhow::bail!(
                        "Strict config validation error: {}",
                        structural_warnings.join("; ")
                    );
                }
            }
        }
        Ok(loaded)
    }

    /// Merges a full Config layer (user/admin/system/cloud) — all fields merge.
    fn merge_config(base: Config, overlay: Config) -> Config {
        Config {
            model: overlay.model,
            review_model: overlay.review_model.or(base.review_model),
            model_provider: overlay.model_provider.or(base.model_provider),
            model_context_window: overlay.model_context_window,
            model_auto_compact_token_limit: overlay.model_auto_compact_token_limit,
            model_post_turn_compact_threshold_percent: overlay
                .model_post_turn_compact_threshold_percent,
            api_base: overlay.api_base,
            api_key: overlay.api_key.or(base.api_key),
            sandbox: overlay.sandbox,
            approval: overlay.approval,
            approvals_reviewer: overlay.approvals_reviewer,
            auto_review: overlay.auto_review.or(base.auto_review),
            allow_login_shell: overlay.allow_login_shell,
            project_doc_max_bytes: overlay.project_doc_max_bytes,
            project_doc_fallback_filenames: overlay.project_doc_fallback_filenames,
            tool_output_token_limit: overlay.tool_output_token_limit,
            web_search: overlay.web_search,
            profile: overlay.profile.or(base.profile),
            profiles: overlay.profiles.into_iter().chain(base.profiles).collect(),
            tui: overlay.tui,
            notify: overlay.notify.or(base.notify),
            memories: overlay.memories,
            hooks: overlay.hooks,
            skills: overlay.skills,
            agents: overlay.agents,
            goals: overlay.goals,
            features: overlay.features.into_iter().chain(base.features).collect(),
            extra_dirs: {
                let mut v = base.extra_dirs;
                v.extend(overlay.extra_dirs);
                v
            },
            auto_start_daemon: overlay.auto_start_daemon,
            routing_enabled: overlay.routing_enabled,
            reasoning_effort: overlay.reasoning_effort.or(base.reasoning_effort),
            mcp_servers: overlay
                .mcp_servers
                .into_iter()
                .chain(base.mcp_servers)
                .collect(),
            mcp: overlay.mcp,
        }
    }

    /// Merges a profile layer (only specific fields).
    fn merge_profile(mut base: Config, prof: ConfigProfile) -> Config {
        if let Some(m) = prof.model {
            base.model = m;
        }
        if let Some(s) = prof.sandbox {
            base.sandbox = s;
        }
        if let Some(a) = prof.approval {
            base.approval = a;
        }
        if let Some(b) = prof.api_base {
            base.api_base = b;
        }
        if let Some(re) = prof.reasoning_effort {
            base.reasoning_effort = Some(re);
        }
        if let Some(f) = prof.features {
            base.features.extend(f);
        }
        if let Some(m) = prof.mcp_servers {
            base.mcp_servers.extend(m);
        }
        base
    }

    /// Merges a repo config layer (limited to project-scoped fields).
    fn merge_repo_config(mut base: Config, repo: Config) -> Config {
        base.extra_dirs.extend(repo.extra_dirs);
        for (k, v) in repo.features {
            base.features.insert(k, v);
        }
        for (k, v) in repo.mcp_servers {
            base.mcp_servers.insert(k, v);
        }
        base
    }

    /// Checks if a repo is trusted for config loading (upstream: skip when untrusted).
    fn is_repo_trusted(path: &Path) -> Result<bool> {
        let trust_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".m31a")
            .join("trust");
        if !trust_dir.exists() {
            return Ok(false);
        }
        let repo_root = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path.parent().unwrap())
            .output()?;
        if !repo_root.status.success() {
            return Ok(false);
        }
        let root = String::from_utf8_lossy(&repo_root.stdout)
            .trim()
            .to_string();
        let mut hasher = Sha256::new();
        hasher.update(root.as_bytes());
        let trust_file = trust_dir.join(format!("{:x}.trust", hasher.finalize()));
        Ok(trust_file.exists())
    }

    pub fn save(&self) -> Result<()> {
        Self::save_to(self, &Self::config_path())
    }

    /// Atomically persists configuration to the specified destination path.
    ///
    /// Ensures:
    /// 1. Serialize the candidate configuration.
    /// 2. Validate the resulting configuration via deserialization.
    /// 3. Write to a temporary file in the same directory.
    /// 4. Flush and synchronize the temporary file.
    /// 5. Atomically replace the destination file.
    /// 6. Synchronize the parent directory where supported.
    /// 7. Preserve appropriate file permissions (0o600 on Unix).
    /// 8. Clean up abandoned temporary files on failure.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "Configuration path '{}' has no parent directory",
                path.display()
            )
        })?;
        fs::create_dir_all(parent)?;

        // 1. Serialize candidate configuration
        let text =
            toml::to_string_pretty(self).context("Failed to serialize configuration to TOML")?;

        // 2. Validate resulting serialized string
        let _: toml::Value = toml::from_str(&text)
            .context("Serialized configuration failed syntactic TOML validation")?;

        // 3. Write to temporary file in the same directory
        let temp_path = parent.join(format!(
            ".config.toml.tmp.{}",
            uuid::Uuid::new_v4().simple()
        ));

        let write_and_sync = || -> Result<()> {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .with_context(|| {
                    format!("Failed to create temporary file '{}'", temp_path.display())
                })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = file.metadata()?.permissions();
                perms.set_mode(0o600);
                let _ = file.set_permissions(perms);
            }

            file.write_all(text.as_bytes())
                .context("Failed to write configuration bytes to temporary file")?;
            file.sync_all()
                .context("Failed to fsync temporary configuration file")?;
            Ok(())
        };

        if let Err(e) = write_and_sync() {
            let _ = fs::remove_file(&temp_path);
            return Err(e);
        }

        // 5. Atomically replace destination
        if let Err(e) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(e).with_context(|| {
                format!(
                    "Failed to atomically rename temporary file '{}' to '{}'",
                    temp_path.display(),
                    path.display()
                )
            });
        }

        // 6. Synchronize parent directory where supported
        #[cfg(unix)]
        {
            if let Ok(dir_file) = fs::File::open(parent) {
                let _ = dir_file.sync_all();
            }
        }

        Ok(())
    }

    pub fn apply_override(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "model" => self.model = value.to_string(),
            "review_model" => self.review_model = Some(value.to_string()),
            "api_base" => self.api_base = value.to_string(),
            "api_key" => self.api_key = Some(value.to_string()),
            "sandbox" | "sandbox_mode" => {
                self.sandbox = match value {
                    "read-only" | "read_only" => SandboxPolicy::ReadOnly,
                    "danger-full-access" | "danger_full_access" | "full" => {
                        SandboxPolicy::DangerFullAccess
                    }
                    _ => SandboxPolicy::WorkspaceWrite,
                };
            }
            "approval" | "approval_policy" => {
                self.approval = match value {
                    "never" => ApprovalPolicy::Never,
                    "approve-for-me" | "auto_review" => ApprovalPolicy::ApproveForMe,
                    _ => ApprovalPolicy::OnRequest,
                };
            }
            "approvals_reviewer" => {
                self.approvals_reviewer = value.to_string();
                if value == "auto_review" {
                    self.approval = ApprovalPolicy::ApproveForMe;
                }
            }
            "allow_login_shell" => {
                self.allow_login_shell = value.parse::<bool>().unwrap_or(true);
            }
            "web_search" => {
                self.web_search = value.to_string();
            }
            "routing_enabled" => {
                self.routing_enabled = value.parse::<bool>().unwrap_or(false);
            }
            "tui.theme" | "theme" => {
                self.tui.theme = Some(value.to_string());
            }
            "tui.animations" => {
                self.tui.animations = value.parse::<bool>().unwrap_or(true);
            }
            "tui.vim_mode_default" => {
                self.tui.vim_mode_default = value.parse::<bool>().unwrap_or(false);
            }
            k if k.starts_with("features.") => {
                let feat = k.trim_start_matches("features.");
                let canonical = match crate::features::FeatureKey::from_str_name(feat) {
                    Some(key) => key.as_str().to_string(),
                    None => match crate::features::resolve_legacy_feature_key(feat) {
                        Some((key, name)) => {
                            tracing::warn!(
                                "Legacy feature key `{feat}` is deprecated; honoring it as `{name}` (successor: {:?}).",
                                key.as_str()
                            );
                            name.to_string()
                        }
                        None => {
                            tracing::warn!(
                                "Unknown feature key `{feat}` in override `-c features.{feat}` (no matching FeatureKey; stored but not enforced)."
                            );
                            feat.to_string()
                        }
                    },
                };
                let val = value.parse::<bool>().unwrap_or(true);
                self.features.insert(canonical, val);
            }
            _ => {
                tracing::warn!("Unrecognized config override: {}", key);
            }
        }
        Ok(())
    }
}

fn find_project_config() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        let candidate = cur.join(".m31a").join("config.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        let git_dir = cur.join(".git");
        if git_dir.exists() {
            let git_cand = cur.join(".m31a").join("config.toml");
            if git_cand.is_file() {
                return Some(git_cand);
            }
            return None;
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

/// Resolves API credentials following hierarchical precedence (environment variables, auth vault, config file fallback):
/// 1. Environment variables (`NVIDIA_API_KEY`, `NVIDIA_NIM_API_KEY`)
/// 2. Encrypted auth vault (`auth.vault` populated by `m31a login`)
/// 3. Secure credentials file (`~/.config/m31a/credentials.json`)
pub fn resolve_stored_api_key() -> Option<String> {
    // 1. Process environment variable
    if let Ok(key) =
        std::env::var("NVIDIA_API_KEY").or_else(|_| std::env::var("NVIDIA_NIM_API_KEY"))
    {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // 2. Encrypted auth vault populated by `m31a login`
    let config_dir = Config::config_dir();
    let vault_dir = config_dir.join("vault");
    if let Ok(vault) = crate::secrets::LocalEncryptedVault::new(vault_dir) {
        use crate::secrets::SecretsBackend;
        if let Ok(Some(secret)) = vault.get_secret(
            crate::secrets::LocalSecretsNamespace::M31aAuth,
            "NVIDIA_API_KEY",
        ) {
            let trimmed = secret.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 3. File fallback credentials (e.g. ~/.config/m31a/credentials.json, mode 0600)
    let creds_file = config_dir.join("credentials.json");
    if let Ok(content) = fs::read_to_string(creds_file)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(key) = v
            .get("nvidia_nim")
            .or_else(|| v.get("api_key"))
            .and_then(|k| k.as_str())
    {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_config_validation_accepts_valid() {
        let toml_valid = r#"
model = "nvidia/nemotron-3.5-lightning-30b-a3b"
sandbox = "workspace-write"
approval = "on-request"
temperature = 0.7
"#;
        let res: Result<Config> = Config::load_layer(toml_valid, true);
        assert!(res.is_ok());
    }

    #[test]
    fn test_strict_config_validation_rejects_unknown_field() {
        let toml_unknown = r#"
model = "nvidia/nemotron-3.5-lightning-30b-a3b"
completely_unknown_setting = 42
"#;
        let res: Result<Config> = Config::load_layer(toml_unknown, true);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown top-level configuration key 'completely_unknown_setting'")
        );
    }

    #[test]
    fn test_strict_config_validation_rejects_invalid_values() {
        let toml_bad_temp = r#"
temperature = 5.0
"#;
        let res: Result<Config> = Config::load_layer(toml_bad_temp, true);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Temperature 5 outside recommended range"));
    }
}
