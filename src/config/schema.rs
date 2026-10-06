//! Strict Configuration Schema & Validation (CFG-02, D-13).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Actionable errors encountered during configuration schema validation.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConfigValidationError {
    #[error("Invalid field '{field}': {message}")]
    InvalidField { field: String, message: String },
    #[error("Unknown field: {message} at {location}")]
    UnknownField { message: String, location: String },
    #[error("Parse error: {message}")]
    ParseError { message: String },
    #[error("Plugin '{plugin_id}' config validation failed: {message}")]
    PluginConfigInvalid { plugin_id: String, message: String },
}

fn default_concurrency_limit() -> usize {
    4
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_sandbox_mode() -> String {
    "standard".to_string()
}

fn default_true() -> bool {
    true
}

fn default_default_action() -> String {
    "ask".to_string()
}

fn default_model() -> String {
    "meta/llama-3.2-11b-vision-instruct".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_fps() -> u32 {
    30
}

fn default_theme() -> String {
    "dark-slate-cyan".to_string()
}

fn default_retention_policy() -> String {
    "keep_on_failure".to_string()
}

fn default_branch_prefix() -> String {
    "m31a/mission".to_string()
}

fn default_provider() -> String {
    "nvidia_nim".to_string()
}

fn default_ignored_paths() -> Vec<String> {
    vec![
        ".git".to_string(),
        "target".to_string(),
        "node_modules".to_string(),
        ".m31a".to_string(),
    ]
}

fn default_research_concurrency() -> usize {
    4
}

fn default_max_discovery_turns() -> usize {
    4
}

fn default_ambiguity_threshold() -> u8 {
    15
}

fn default_false() -> bool {
    false
}

fn default_projection_dir() -> String {
    ".planning".to_string()
}

/// Runtime subsystem configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default = "default_concurrency_limit")]
    pub concurrency_limit: usize,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_sandbox_mode")]
    pub sandbox_mode: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            concurrency_limit: default_concurrency_limit(),
            timeout_secs: default_timeout_secs(),
            sandbox_mode: default_sandbox_mode(),
        }
    }
}

/// Canonical git push policy (typed contract; serialization uses exact
/// canonical values `allow`/`ask`/`deny`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GitPushPolicy {
    Allow,
    #[default]
    Ask,
    Deny,
}

impl GitPushPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

impl std::str::FromStr for GitPushPolicy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "allow" => Ok(Self::Allow),
            "ask" => Ok(Self::Ask),
            "deny" => Ok(Self::Deny),
            other => Err(format!(
                "invalid git.push_policy '{other}': expected one of allow, ask, deny"
            )),
        }
    }
}

impl std::fmt::Display for GitPushPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Canonical git execution isolation (typed contract; `required` fail-closed
/// default, `best_effort` explicit opt-in fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GitExecutionIsolation {
    #[default]
    Required,
    BestEffort,
}

impl GitExecutionIsolation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::BestEffort => "best_effort",
        }
    }

    pub fn is_required(&self) -> bool {
        matches!(self, Self::Required)
    }
}

impl std::str::FromStr for GitExecutionIsolation {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "required" => Ok(Self::Required),
            "best_effort" | "besteffort" => Ok(Self::BestEffort),
            other => Err(format!(
                "invalid git.execution_isolation '{other}': expected one of required, best_effort"
            )),
        }
    }
}

impl std::fmt::Display for GitExecutionIsolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Git subsystem configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub worktree_dir: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub auto_commit: bool,
    #[serde(default)]
    pub push_policy: GitPushPolicy,
    #[serde(default = "default_retention_policy")]
    pub retention_policy: String,
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    /// Execution isolation policy for autonomous governed missions.
    ///
    /// - `required` (default): worktree creation failure blocks execution. No autonomous
    ///   governed execution may proceed in the primary workspace without explicit
    ///   operator policy override. Fails closed.
    /// - `best_effort`: create a git worktree if possible; fall back to
    ///   primary workspace with an explicit log warning. Explicit opt-in only.
    #[serde(default)]
    pub execution_isolation: GitExecutionIsolation,
}

fn default_execution_isolation() -> GitExecutionIsolation {
    GitExecutionIsolation::Required
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            worktree_dir: None,
            auto_commit: default_true(),
            push_policy: GitPushPolicy::Ask,
            retention_policy: default_retention_policy(),
            branch_prefix: default_branch_prefix(),
            execution_isolation: default_execution_isolation(),
        }
    }
}

/// Policy and security gate configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default = "default_default_action")]
    pub default_action: String,
    #[serde(default = "default_true", alias = "require_approval_for_destructive")]
    pub interactive_approvals: bool,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub sandbox_mode: Option<String>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            default_action: default_default_action(),
            interactive_approvals: default_true(),
            denied_tools: Vec::new(),
            sandbox_mode: None,
        }
    }
}

/// Provider-specific connection and model options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderTableConfig {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
}

/// Provider selection and catalog configuration.
///
/// NVIDIA NIM is the production model provider in the current release.
/// Retired per-provider tables (`openai`, `anthropic`, `gemini`, `local`)
/// are retained in the schema solely so legacy configuration files parse;
/// [`validate_config`] rejects any attempt to select or configure them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    #[serde(default = "default_provider")]
    pub default: String,
    #[serde(default)]
    pub nvidia_nim: Option<ProviderTableConfig>,
    #[serde(default)]
    pub openai: Option<ProviderTableConfig>,
    #[serde(default)]
    pub anthropic: Option<ProviderTableConfig>,
    #[serde(default)]
    pub gemini: Option<ProviderTableConfig>,
    #[serde(default)]
    pub local: Option<ProviderTableConfig>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            default: default_provider(),
            nvidia_nim: Some(ProviderTableConfig {
                base_url: Some("https://integrate.api.nvidia.com/v1".to_string()),
                default_model: Some("meta/llama-3.2-11b-vision-instruct".to_string()),
            }),
            openai: None,
            anthropic: None,
            gemini: None,
            local: None,
        }
    }
}

/// Verification command overrides for workspace verification gates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceVerificationConfig {
    #[serde(default, alias = "manifest_file", alias = "manifest")]
    pub tier1_manifest: Option<String>,
    #[serde(default, alias = "compiler_command", alias = "compiler")]
    pub tier2_compiler: Option<String>,
    #[serde(default, alias = "test_command", alias = "tests")]
    pub tier3_tests: Option<String>,
    #[serde(default, alias = "linter_command", alias = "linter")]
    pub tier4_linter: Option<String>,
    #[serde(default, alias = "timeout")]
    pub timeout_secs: Option<u64>,
}

impl WorkspaceVerificationConfig {
    pub fn test_command(&self) -> Option<&str> {
        self.tier3_tests.as_deref()
    }
}

/// Workspace-specific project settings and ignore paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub project_type: Option<String>,
    #[serde(default = "default_ignored_paths")]
    pub ignored_paths: Vec<String>,
    #[serde(default)]
    pub verification: Option<WorkspaceVerificationConfig>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            project_type: None,
            ignored_paths: default_ignored_paths(),
            verification: None,
        }
    }
}

/// Autonomous execution budget ceilings preventing runaway execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct BudgetConfig {
    #[serde(default)]
    pub max_agent_steps: Option<usize>,
    #[serde(default)]
    pub max_model_calls: Option<usize>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
    #[serde(default)]
    pub max_wall_clock_seconds: Option<u64>,
    #[serde(default)]
    pub max_retries: Option<usize>,
}

/// Agents and model routing configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentsConfig {
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_auxiliary_model: Option<String>,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub provider: Option<String>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            default_model: default_model(),
            fast_auxiliary_model: None,
            fallback_models: Vec::new(),
            max_tokens: default_max_tokens(),
            provider: None,
        }
    }
}

/// TUI and console display configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TuiConfig {
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub compact_mode: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            theme: default_theme(),
            compact_mode: false,
        }
    }
}

/// Authoritative workflow subsystem configuration options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    /// Enable or disable domain research before planning.
    #[serde(default = "default_true")]
    pub research: bool,

    /// Concurrency limit for parallel research dimensions (1..8).
    #[serde(default = "default_research_concurrency")]
    pub research_concurrency: usize,

    /// Maximum interactive question turns during discovery (1..10).
    #[serde(default = "default_max_discovery_turns")]
    pub max_discovery_turns: usize,

    /// Ambiguity threshold to conclude discovery (1..=100).
    #[serde(default = "default_ambiguity_threshold")]
    pub ambiguity_threshold_percent: u8,

    /// Require human approval before finalizing Project Charter.
    #[serde(default = "default_true")]
    pub require_charter_approval: bool,

    /// Require human approval before finalizing Architecture & ADRs.
    #[serde(default = "default_true")]
    pub require_architecture_approval: bool,

    /// Require human approval before finalizing Roadmap.
    #[serde(default = "default_true")]
    pub require_roadmap_approval: bool,

    /// Automatically advance from Genesis directly into initial roadmap phase execution.
    #[serde(default = "default_false")]
    pub auto_advance: bool,

    /// Projection directory for human-readable planning artifacts (.planning).
    #[serde(default = "default_projection_dir")]
    pub projection_dir: String,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            research: default_true(),
            research_concurrency: default_research_concurrency(),
            max_discovery_turns: default_max_discovery_turns(),
            ambiguity_threshold_percent: default_ambiguity_threshold(),
            require_charter_approval: default_true(),
            require_architecture_approval: default_true(),
            require_roadmap_approval: default_true(),
            auto_advance: default_false(),
            projection_dir: default_projection_dir(),
        }
    }
}

/// Root Application Configuration strictly enforcing schema and namespacing (D-13).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Active execution profile name (e.g. "balanced", "autonomous", "conservative", "code_reviewer")
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "active_profile"
    )]
    pub profile: Option<String>,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub budget: BudgetConfig,
    #[serde(default)]
    pub tui: TuiConfig,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    /// Quarantined, namespaced configuration tables for plugins: [plugins.<id>]
    #[serde(default)]
    pub plugins: HashMap<String, toml::Value>,
    /// Composable named profiles: [profiles.<name>]
    #[serde(default)]
    pub profiles: HashMap<String, toml::Value>,
}

/// Validate logical constraints across all configuration fields.
pub fn validate_config(config: &AppConfig) -> Result<(), ConfigValidationError> {
    // 1. Runtime bounds
    if config.runtime.concurrency_limit == 0 || config.runtime.concurrency_limit > 32 {
        return Err(ConfigValidationError::InvalidField {
            field: "runtime.concurrency_limit".to_string(),
            message: "concurrency_limit must be between 1 and 32".to_string(),
        });
    }
    if config.runtime.timeout_secs == 0 || config.runtime.timeout_secs > 86400 {
        return Err(ConfigValidationError::InvalidField {
            field: "runtime.timeout_secs".to_string(),
            message: "timeout_secs must be between 1 and 86400 (24h)".to_string(),
        });
    }
    let valid_sandboxes = ["strict", "standard", "permissive", "disabled"];
    if !valid_sandboxes.contains(&config.runtime.sandbox_mode.as_str()) {
        return Err(ConfigValidationError::InvalidField {
            field: "runtime.sandbox_mode".to_string(),
            message: format!(
                "sandbox_mode must be one of {:?}, got '{}'",
                valid_sandboxes, config.runtime.sandbox_mode
            ),
        });
    }

    // 2. Git bounds
    let valid_push_policies = ["allow", "ask", "deny"];
    if !valid_push_policies.contains(&config.git.push_policy.as_str()) {
        return Err(ConfigValidationError::InvalidField {
            field: "git.push_policy".to_string(),
            message: format!(
                "push_policy must be one of {:?}, got '{}'",
                valid_push_policies, config.git.push_policy
            ),
        });
    }

    // 3. Policy bounds
    let valid_actions = ["allow", "ask", "deny"];
    if !valid_actions.contains(&config.policy.default_action.as_str()) {
        return Err(ConfigValidationError::InvalidField {
            field: "policy.default_action".to_string(),
            message: format!(
                "default_action must be one of {:?}, got '{}'",
                valid_actions, config.policy.default_action
            ),
        });
    }

    // 4. Agents bounds
    if config.agents.default_model.trim().is_empty() {
        return Err(ConfigValidationError::InvalidField {
            field: "agents.default_model".to_string(),
            message: "default_model cannot be empty".to_string(),
        });
    }
    // Model IDs carrying a retired provider qualifier (e.g.
    // `openai/gpt-4o`) must fail deterministically instead of silently
    // becoming the active runtime model.
    if let Some((prov, _)) = config.agents.default_model.trim().split_once('/')
        && crate::config::provider_registry::is_retired_provider(prov)
    {
        return Err(ConfigValidationError::InvalidField {
            field: "agents.default_model".to_string(),
            message: format!(
                "unsupported model provider '{}'. {}",
                prov,
                crate::config::provider_registry::NVIDIA_ONLY_ERROR
            ),
        });
    }
    if config.agents.max_tokens < 256 || config.agents.max_tokens > 200_000 {
        return Err(ConfigValidationError::InvalidField {
            field: "agents.max_tokens".to_string(),
            message: "max_tokens must be between 256 and 200000".to_string(),
        });
    }

    // 5. TUI bounds
    if config.tui.fps < 1 || config.tui.fps > 120 {
        return Err(ConfigValidationError::InvalidField {
            field: "tui.fps".to_string(),
            message: "fps must be between 1 and 120".to_string(),
        });
    }

    // 6. Git retention bounds
    let valid_retention = ["keep_on_failure", "always_remove", "always_keep"];
    if !valid_retention.contains(&config.git.retention_policy.as_str()) {
        return Err(ConfigValidationError::InvalidField {
            field: "git.retention_policy".to_string(),
            message: format!(
                "retention_policy must be one of {:?}, got '{}'",
                valid_retention, config.git.retention_policy
            ),
        });
    }

    // 7. Provider bounds: NVIDIA NIM is the production model provider.
    if config.provider.default.trim().is_empty() {
        return Err(ConfigValidationError::InvalidField {
            field: "provider.default".to_string(),
            message: "default provider cannot be empty".to_string(),
        });
    }
    {
        let normalized =
            crate::config::provider_registry::normalize_provider_id(config.provider.default.trim());
        if normalized != crate::config::provider_registry::PRODUCTION_PROVIDER_ID {
            return Err(ConfigValidationError::InvalidField {
                field: "provider.default".to_string(),
                message: format!(
                    "unsupported provider '{}'. {}",
                    config.provider.default.trim(),
                    crate::config::provider_registry::NVIDIA_ONLY_ERROR
                ),
            });
        }
        for retired in ["openai", "anthropic", "gemini", "local"] {
            let configured = match retired {
                "openai" => &config.provider.openai,
                "anthropic" => &config.provider.anthropic,
                "gemini" => &config.provider.gemini,
                _ => &config.provider.local,
            };
            if configured.is_some() {
                return Err(ConfigValidationError::InvalidField {
                    field: format!("provider.{retired}"),
                    message: format!(
                        "provider '{retired}' is retired in this release. NVIDIA NIM is the production model provider."
                    ),
                });
            }
        }
    }

    // 8. Workspace bounds
    if let Some(ref pt) = config.workspace.project_type {
        let valid_types = ["rust", "python", "node", "go", "custom"];
        if !valid_types.contains(&pt.as_str()) {
            return Err(ConfigValidationError::InvalidField {
                field: "workspace.project_type".to_string(),
                message: format!(
                    "project_type must be one of {:?}, got '{}'",
                    valid_types, pt
                ),
            });
        }
    }

    // 9. Budget bounds
    if let Some(steps) = config.budget.max_agent_steps
        && steps == 0
    {
        return Err(ConfigValidationError::InvalidField {
            field: "budget.max_agent_steps".to_string(),
            message: "max_agent_steps must be greater than 0".to_string(),
        });
    }
    if let Some(calls) = config.budget.max_model_calls
        && calls == 0
    {
        return Err(ConfigValidationError::InvalidField {
            field: "budget.max_model_calls".to_string(),
            message: "max_model_calls must be greater than 0".to_string(),
        });
    }
    if let Some(tokens) = config.budget.max_tokens
        && tokens == 0
    {
        return Err(ConfigValidationError::InvalidField {
            field: "budget.max_tokens".to_string(),
            message: "max_tokens must be greater than 0".to_string(),
        });
    }
    if let Some(secs) = config.budget.max_wall_clock_seconds
        && secs == 0
    {
        return Err(ConfigValidationError::InvalidField {
            field: "budget.max_wall_clock_seconds".to_string(),
            message: "max_wall_clock_seconds must be greater than 0".to_string(),
        });
    }
    if let Some(cost) = config.budget.max_cost_usd
        && cost <= 0.0
    {
        return Err(ConfigValidationError::InvalidField {
            field: "budget.max_cost_usd".to_string(),
            message: "max_cost_usd must be greater than 0.0".to_string(),
        });
    }

    // 10. Workflow bounds
    if config.workflow.research_concurrency == 0 || config.workflow.research_concurrency > 8 {
        return Err(ConfigValidationError::InvalidField {
            field: "workflow.research_concurrency".to_string(),
            message: "research_concurrency must be between 1 and 8".to_string(),
        });
    }
    if config.workflow.max_discovery_turns == 0 || config.workflow.max_discovery_turns > 10 {
        return Err(ConfigValidationError::InvalidField {
            field: "workflow.max_discovery_turns".to_string(),
            message: "max_discovery_turns must be between 1 and 10".to_string(),
        });
    }
    if config.workflow.ambiguity_threshold_percent == 0
        || config.workflow.ambiguity_threshold_percent > 100
    {
        return Err(ConfigValidationError::InvalidField {
            field: "workflow.ambiguity_threshold_percent".to_string(),
            message: "ambiguity_threshold_percent must be between 1 and 100".to_string(),
        });
    }
    if config.workflow.projection_dir.trim().is_empty() {
        return Err(ConfigValidationError::InvalidField {
            field: "workflow.projection_dir".to_string(),
            message: "projection_dir cannot be empty".to_string(),
        });
    }

    Ok(())
}

/// Parse and validate a TOML configuration string, catching unknown fields and logical errors.
pub fn parse_and_validate_config(toml_str: &str) -> Result<AppConfig, ConfigValidationError> {
    let config: AppConfig = toml::from_str(toml_str).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unknown field") {
            let span = e.span();
            let location = if let Some(s) = span {
                format!("offset {}-{}", s.start, s.end)
            } else {
                "unknown location".to_string()
            };
            ConfigValidationError::UnknownField {
                message: msg,
                location,
            }
        } else {
            ConfigValidationError::ParseError { message: msg }
        }
    })?;

    validate_config(&config)?;
    Ok(config)
}
