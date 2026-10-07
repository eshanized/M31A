//! 7-Step Guided Onboarding Setup Wizard Screen (FRX-02, FRX-03, D-02).
//!
//! Interactive terminal wizard walking the operator through Workspace Trust,
//! Doctor Diagnostics, Provider Selection, Model Selection, Profile Selection,
//! Autonomy/Safety constraints, and Final Verification.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::provider_registry::ProviderRegistry;
use crate::config::schema::AppConfig;
use crate::deployment::DeploymentChannel;
use crate::init::doctor::{DiagnosticProbe, DiagnosticStatus, DoctorEngine};
use crate::init::lifecycle::SetupStep;
use crate::model::catalog::{CatalogRefreshState, CatalogSource, ModelCatalog};
use crate::model::provider::EndpointTrustSource;
use crate::model::provider::nvidia::NvidiaProvider;
use crate::model::router::resolver::{ModelCandidate, ModelTier};
use crate::model::types::{ModelError, ProviderCapabilityStatus};
use crate::runtime_authorities::{CredentialSource, resolve_runtime_credentials};
use crate::tui::input::text_input::{InputMasking, TextInput};
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Outcome of handling a key event in the setup wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardOutcome {
    None,
    Advanced(SetupStep),
    Back(SetupStep),
    Cancelled,
    Completed,
    Error(String),
}

/// Execution profile choices for Step 5 (canonical profile universe).
/// The canonical `ProfileResolver` is authoritative; this enum is a
/// projection of its ids (never a second universe). Persistence writes the
/// canonical id and runtime resolves the same definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardProfile {
    Balanced,
    Autonomous,
    Conservative,
    CodeReviewer,
    Safe,
    Coding,
    Research,
    Ci,
    SecurityReview,
    Release,
}

impl WizardProfile {
    /// All canonical profiles in resolver order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Balanced,
            Self::Autonomous,
            Self::Conservative,
            Self::CodeReviewer,
            Self::Safe,
            Self::Coding,
            Self::Research,
            Self::Ci,
            Self::SecurityReview,
            Self::Release,
        ]
    }

    /// Canonical profile id persisted to config and resolved at runtime.
    pub fn canonical_id(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Autonomous => "autonomous",
            Self::Conservative => "conservative",
            Self::CodeReviewer => "code_reviewer",
            Self::Safe => "safe",
            Self::Coding => "coding",
            Self::Research => "research",
            Self::Ci => "ci",
            Self::SecurityReview => "security_review",
            Self::Release => "release",
        }
    }

    /// Parse a canonical id (or hyphen alias) into a wizard selection.
    /// Unknown ids fail closed to `None` so callers report explicitly
    /// instead of silently converting to balanced.
    pub fn from_canonical_id(id: &str) -> Option<Self> {
        match id.trim().to_lowercase().replace('-', "_").as_str() {
            "balanced" => Some(Self::Balanced),
            "autonomous" => Some(Self::Autonomous),
            "conservative" => Some(Self::Conservative),
            "code_reviewer" => Some(Self::CodeReviewer),
            "safe" => Some(Self::Safe),
            "coding" => Some(Self::Coding),
            "research" => Some(Self::Research),
            "ci" => Some(Self::Ci),
            "security_review" => Some(Self::SecurityReview),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Autonomous => "Autonomous",
            Self::Conservative => "Conservative",
            Self::CodeReviewer => "Code Reviewer",
            Self::Safe => "Safe",
            Self::Coding => "Coding",
            Self::Research => "Research",
            Self::Ci => "CI",
            Self::SecurityReview => "Security Review",
            Self::Release => "Release",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Balanced => {
                "Standard dual-agent workflow with bounded iterations and review checkpoints."
            }
            Self::Autonomous => {
                "Higher autonomy threshold, auto-approving read/dry-run tool invocations."
            }
            Self::Conservative => {
                "Strict human authorization required for every filesystem modification or process execution."
            }
            Self::CodeReviewer => {
                "Read-only inspection and critique profile; side effects disabled."
            }
            Self::Safe => "Minimal latitude; every mutating action requires approval.",
            Self::Coding => "Assisted coding with workspace writes and test execution.",
            Self::Research => "Read-only research and discovery; writes denied.",
            Self::Ci => "Unattended CI execution with strict sandboxing.",
            Self::SecurityReview => "Read-only security audit with highest verification tier.",
            Self::Release => "Assisted release packaging with governed git operations.",
        }
    }

    pub fn recommendation(&self) -> &'static str {
        match self {
            Self::Balanced => "Recommended for most software engineering workflows.",
            Self::Autonomous => {
                "Recommended for trusted repositories with comprehensive test suites."
            }
            Self::Conservative => "Recommended for sensitive repositories and initial exploration.",
            Self::CodeReviewer => "Recommended for non-invasive audit and code review tasks.",
            Self::Safe => "Recommended when maximum oversight is required.",
            Self::Coding => "Recommended for day-to-day implementation work.",
            Self::Research => "Recommended for exploration and architecture discovery.",
            Self::Ci => "Recommended for non-interactive continuous integration.",
            Self::SecurityReview => "Recommended for security audits.",
            Self::Release => "Recommended for release preparation.",
        }
    }

    /// Canonical autonomy mode owning this profile (resolver is source).
    pub fn autonomy_mode(&self) -> &'static str {
        match self {
            Self::Balanced | Self::Coding | Self::Release => "assisted",
            Self::Autonomous => "autonomous",
            Self::Ci => "unattended",
            Self::Safe
            | Self::Conservative
            | Self::Research
            | Self::CodeReviewer
            | Self::SecurityReview => "safe",
        }
    }
}

pub use crate::git::GitWorkspaceInfo;

/// Truthful status of provider authentication and network verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderVerificationState {
    Unverified,
    Checking,
    Success {
        latency: Duration,
        model: String,
        endpoint: String,
    },
    AuthenticationFailed {
        status_code: Option<u16>,
        details: String,
    },
    NetworkError(String),
    EndpointBlocked(String),
    ModelUnavailable {
        model: String,
        details: String,
    },
    RateLimited {
        retry_after_secs: Option<u64>,
        details: String,
    },
    Misconfigured(String),
    Degraded {
        latency: Duration,
        reason: String,
    },
}

impl ProviderVerificationState {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. } | Self::Degraded { .. })
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Unverified => "[UNVERIFIED]",
            Self::Checking => "[CHECKING]",
            Self::Success { .. } => "[OPERATIONAL]",
            Self::AuthenticationFailed { .. } => "[AUTH FAILED]",
            Self::NetworkError(_) => "[NETWORK ERR]",
            Self::EndpointBlocked(_) => "[BLOCKED]",
            Self::ModelUnavailable { .. } => "[UNAVAILABLE]",
            Self::RateLimited { .. } => "[RATE LIMIT]",
            Self::Misconfigured(_) => "[MISCONFIGURED]",
            Self::Degraded { .. } => "[DEGRADED]",
        }
    }
}

/// Interactive 7-Step Setup Wizard Screen component.
#[derive(Debug)]
pub struct SetupWizardScreen {
    workspace_path: PathBuf,
    current_step: SetupStep,
    pub doctor: DoctorEngine,
    probes: Vec<DiagnosticProbe>,

    // Step 1: Trust & Git integration
    pub trust_confirmed: bool,
    pub git_info: GitWorkspaceInfo,
    pub git_enabled: bool,
    pub git_auto_commit: bool,
    pub git_push_policy: crate::config::schema::GitPushPolicy,
    pub git_execution_isolation: crate::config::schema::GitExecutionIsolation,
    pub git_branch_prefix: String,

    // Step 2: Doctor
    pub warnings_acknowledged: bool,

    // Step 3: Provider
    pub api_key_input: TextInput,
    pub base_url: Option<String>,
    pub initial_credential_source: CredentialSource,
    pub step3_connection_state: ProviderVerificationState,

    // Step 4: Model Catalog, Search & Role Assignments
    pub primary_model_input: TextInput,
    pub fast_model_input: TextInput,
    pub catalog: ModelCatalog,
    pub catalog_verified_live: bool,
    pub model_search_input: TextInput,
    pub selected_model_index: usize,
    pub model_scroll_offset: usize,
    pub model_focus_search: bool,

    // Step 5: Profile & Workflow
    pub profile: WizardProfile,
    pub workflow_research: bool,
    pub workflow_auto_advance: bool,

    // Step 6: Autonomy & Resources ("Free Coding")
    pub require_approval_for_writes: bool,
    pub sandbox_mode: String,
    pub unlimited_budget: bool,
    pub max_budget_dollars: u32,
    pub max_agent_steps: Option<usize>,
    pub max_tokens: Option<u64>,
    pub max_model_calls: Option<usize>,
    pub max_wall_clock_seconds: Option<u64>,
    pub max_retries: Option<usize>,
    pub concurrency_limit: usize,

    // Configuration scope: where settings persist.
    //
    // - `Workspace` (default, backward compatible): `<ws>/.m31a/config.toml`
    //   holds THIS workspace's settings.
    // - `Global`: the platform user config (`~/.config/m31a/config.toml`)
    //   holds M31A-wide settings reusable across projects.
    // Credentials ALWAYS go to the global user store; cache ALWAYS goes to
    // the platform cache; only the TOML config honors this scope.
    pub config_scope: crate::storage::ConfigScope,

    // Step 7: Verification
    pub verification_state: ProviderVerificationState,
    pub connection_tested: bool,
    pub connection_status: Option<String>,

    pub theme: ThemeTokens,
    // canonical theme selection (single authority: AppConfig.tui.theme).
    // `theme` tokens are a projection of this mode for rendering.
    pub theme_mode: ThemeMode,
    pub status_message: Option<String>,
}

impl SetupWizardScreen {
    /// Display-only wizard prefill for the budget input when no budget is
    /// configured. ALGORITHMIC/UI constant: never a runtime policy and never
    /// persisted unless the user explicitly confirms it.
    pub const WIZARD_DISPLAY_DEFAULT_BUDGET_DOLLARS: u32 = 25;
    pub const PROVIDERS: &'static [&'static str] = &[
        "NVIDIA NIM (Dynamic Model Discovery) [AVAILABLE]",
        "Anthropic (Claude 3.5 Sonnet) [UNAVAILABLE - Deferred in v1]",
        "OpenAI (GPT-4o) [UNAVAILABLE - Deferred in v1]",
        "Google Gemini (1.5 Pro) [UNAVAILABLE - Deferred in v1]",
        "OpenAI-Compatible (Local / Ollama) [UNAVAILABLE - Deferred in v1]",
    ];

    pub fn new(workspace_path: PathBuf) -> Self {
        let doctor = DoctorEngine::new();
        let git_info = GitWorkspaceInfo::probe(&workspace_path);

        // Prefill from the effective configuration: workspace file wins when
        // present, otherwise the global user config (so returning users see
        // their M31A-wide prefs in every new workspace).
        let layout = crate::storage::StorageLayout::for_workspace(&workspace_path);
        let m31a_config_path = workspace_path.join(".m31a").join("config.toml");
        let existing_config = if m31a_config_path.is_file() {
            std::fs::read_to_string(&m31a_config_path)
                .ok()
                .and_then(|c| crate::config::schema::parse_and_validate_config(&c).ok())
        } else {
            let global = layout.user_config_file();
            if global.is_file() {
                std::fs::read_to_string(&global)
                    .ok()
                    .and_then(|c| crate::config::schema::parse_and_validate_config(&c).ok())
            } else {
                None
            }
        };

        let git_enabled = existing_config
            .as_ref()
            .map(|c| c.git.enabled)
            .unwrap_or(git_info.is_git_repo);

        let probes = doctor.run_all_with_git_enabled(&workspace_path, git_enabled);

        let git_auto_commit = existing_config
            .as_ref()
            .map(|c| c.git.auto_commit)
            .unwrap_or(true);
        let git_push_policy = existing_config
            .as_ref()
            .map(|c| c.git.push_policy)
            .unwrap_or(crate::config::schema::GitPushPolicy::Ask);
        let git_execution_isolation = existing_config
            .as_ref()
            .map(|c| c.git.execution_isolation)
            .unwrap_or(crate::config::schema::GitExecutionIsolation::Required);
        let git_branch_prefix = existing_config
            .as_ref()
            .map(|c| c.git.branch_prefix.clone())
            .unwrap_or_else(|| crate::config::canonical::DEFAULT_BRANCH_PREFIX.to_string());

        let mut api_key_input = TextInput::single_line().with_masking(InputMasking::Masked('*'));

        // Resolve credentials through the authoritative unified runtime authority
        let channel = DeploymentChannel::current();
        let cred_resolution = resolve_runtime_credentials(&workspace_path, channel);

        let key_from_resolution = cred_resolution.api_key.clone();
        let initial_credential_source = cred_resolution.source;

        if let Some(ref k) = key_from_resolution
            && !k.trim().is_empty()
        {
            api_key_input.set_text(k);
        }

        let mut primary_model_input = TextInput::single_line();
        let mut fast_model_input = TextInput::single_line();
        let model_search_input = TextInput::single_line();

        // canonical catalog authority: global platform cache first,
        // legacy workspace cache only as migration fallback. channel-aware
        // so development never reads production discovery state.
        let layout_for_catalog = crate::storage::StorageLayout::for_workspace(&workspace_path);
        let global_cache_path = layout_for_catalog.global_model_catalog_file();
        let legacy_cache_path =
            ModelCatalog::cache_path_for_channel(&workspace_path, DeploymentChannel::current());
        let mut catalog = ModelCatalog::load_from_cache_file(&global_cache_path)
            .ok()
            .filter(|c| c.schema_version >= ModelCatalog::CURRENT_CATALOG_SCHEMA_VERSION)
            .or_else(|| {
                ModelCatalog::load_from_cache_file(&legacy_cache_path)
                    .ok()
                    .filter(|c| c.schema_version >= ModelCatalog::CURRENT_CATALOG_SCHEMA_VERSION)
            })
            .unwrap_or_else(|| {
                ModelCatalog::new(crate::config::provider_registry::PRODUCTION_PROVIDER_ID)
            });
        let cache_path = global_cache_path;

        let mut catalog_verified_live = false;

        // If catalog was empty and key is present in environment, perform initial discovery
        if catalog.is_empty()
            && key_from_resolution.is_some()
            && let Ok(discovered) = Self::run_discovery_sync(None, key_from_resolution.as_deref())
        {
            catalog.update_from_provider(
                crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
                discovered,
            );
            catalog.source = CatalogSource::Discovered;
            catalog.refresh_state = CatalogRefreshState::DiscoverySuccess;
            let _ = catalog.save_to_cache_file(&cache_path);
            catalog_verified_live = true;
        }

        if let Some(ref c) = existing_config
            && !c.agents.default_model.is_empty()
        {
            primary_model_input.set_text(&c.agents.default_model);
        } else if let Some(def) =
            catalog.select_default(Some(crate::config::canonical::CANONICAL_DEFAULT_MODEL))
        {
            primary_model_input.set_text(&def.model_id);
        } else {
            primary_model_input.set_text(crate::config::canonical::CANONICAL_DEFAULT_MODEL);
        }

        if let Some(ref c) = existing_config
            && let Some(ref fast) = c.agents.fast_auxiliary_model
        {
            fast_model_input.set_text(fast);
        } else if let Some(fast) = catalog.select_fast_default(Some(primary_model_input.text())) {
            fast_model_input.set_text(&fast.model_id);
        } else {
            fast_model_input.set_text(crate::config::canonical::CANONICAL_DEFAULT_MODEL);
        }

        // canonical profile prefill: unknown ids stay visible as balanced
        // selection but the stored config value is never silently rewritten
        // here (persistence writes the canonical id of the selection).
        let profile = existing_config
            .as_ref()
            .and_then(|c| c.profile.as_deref())
            .and_then(WizardProfile::from_canonical_id)
            .unwrap_or(WizardProfile::Balanced);

        let require_approval_for_writes = existing_config
            .as_ref()
            .map(|c| c.policy.interactive_approvals)
            .unwrap_or(true);

        let sandbox_mode = existing_config
            .as_ref()
            .and_then(|c| c.policy.sandbox_mode.clone())
            .unwrap_or_else(|| crate::config::canonical::DEFAULT_SANDBOX_MODE.to_string());

        let unlimited_budget = existing_config
            .as_ref()
            .map(|c| c.budget.max_cost_usd.is_none() && c.budget.max_agent_steps.is_none())
            .unwrap_or(true);

        let max_budget_dollars = existing_config
            .as_ref()
            .and_then(|c| c.budget.max_cost_usd)
            .map(|d| d as u32)
            // Display-only prefill for the wizard input when no budget is
            // configured (unlimited). Never a runtime policy: persistence
            // writes `None` (unlimited) unless the user types a value.
            .unwrap_or(Self::WIZARD_DISPLAY_DEFAULT_BUDGET_DOLLARS);

        let max_agent_steps = existing_config
            .as_ref()
            .and_then(|c| c.budget.max_agent_steps);
        let max_tokens = existing_config.as_ref().and_then(|c| c.budget.max_tokens);
        let max_model_calls = existing_config
            .as_ref()
            .and_then(|c| c.budget.max_model_calls);
        let max_wall_clock_seconds = existing_config
            .as_ref()
            .and_then(|c| c.budget.max_wall_clock_seconds);
        let max_retries = existing_config.as_ref().and_then(|c| c.budget.max_retries);

        let concurrency_limit = existing_config
            .as_ref()
            .map(|c| c.runtime.concurrency_limit)
            .unwrap_or(crate::config::canonical::DEFAULT_RUNTIME_CONCURRENCY);

        let workflow_research = existing_config
            .as_ref()
            .map(|c| c.workflow.research)
            .unwrap_or(true);

        let workflow_auto_advance = existing_config
            .as_ref()
            .map(|c| c.workflow.auto_advance)
            .unwrap_or(false);

        // Canonical theme prefill: existing config owns the selection;
        // absent configuration resolves through the single canonical
        // default (same value the resolver produces), never a second
        // hardcoded variant.
        let theme_mode = existing_config
            .as_ref()
            .map(|c| ThemeMode::from_str_relaxed(&c.tui.theme))
            .unwrap_or_else(ThemeMode::canonical_default);
        let theme = ThemeTokens::resolve(theme_mode);

        Self {
            workspace_path,
            current_step: SetupStep::WorkspaceTrust,
            doctor,
            probes,
            trust_confirmed: false,
            git_info,
            git_enabled,
            git_auto_commit,
            git_push_policy,
            git_execution_isolation,
            git_branch_prefix,
            warnings_acknowledged: false,
            api_key_input,
            base_url: None,
            initial_credential_source,
            step3_connection_state: ProviderVerificationState::Unverified,
            primary_model_input,
            fast_model_input,
            catalog,
            catalog_verified_live,
            model_search_input,
            selected_model_index: 0,
            model_scroll_offset: 0,
            model_focus_search: false,
            profile,
            workflow_research,
            workflow_auto_advance,
            require_approval_for_writes,
            sandbox_mode,
            unlimited_budget,
            max_budget_dollars,
            max_agent_steps,
            max_tokens,
            max_model_calls,
            max_wall_clock_seconds,
            max_retries,
            concurrency_limit,
            config_scope: crate::storage::ConfigScope::Workspace,
            verification_state: ProviderVerificationState::Unverified,
            connection_tested: false,
            connection_status: None,
            theme,
            theme_mode,
            status_message: None,
        }
    }

    /// Set the configuration scope (global user profile vs this workspace).
    pub fn with_config_scope(mut self, scope: crate::storage::ConfigScope) -> Self {
        self.config_scope = scope;
        self
    }

    /// Human-readable description of where settings will be saved.
    pub fn config_scope_label(&self) -> String {
        let layout = crate::storage::StorageLayout::for_workspace(&self.workspace_path);
        match self.config_scope {
            crate::storage::ConfigScope::Global => format!(
                "M31A-wide settings → your M31A user profile ({}) · Credentials → your M31A user store · Cache → platform cache",
                layout.user_config_file().display()
            ),
            crate::storage::ConfigScope::Workspace => format!(
                "Workspace settings → only for this project ({}) · Credentials → your M31A user store · Cache → platform cache",
                layout.workspace_config_file().display()
            ),
        }
    }

    /// Toggle configuration scope between workspace and global.
    pub fn toggle_config_scope(&mut self) {
        self.config_scope = match self.config_scope {
            crate::storage::ConfigScope::Workspace => crate::storage::ConfigScope::Global,
            crate::storage::ConfigScope::Global => crate::storage::ConfigScope::Workspace,
        };
        self.status_message = Some(format!("Configuration scope: {}", self.config_scope));
    }

    /// Set an explicit custom base URL (useful for test doubles and hermetic mocks).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set theme tokens for visual rendering (keeps canonical mode in sync).
    pub fn with_theme(mut self, theme: ThemeTokens) -> Self {
        self.theme_mode = theme.mode;
        self.theme = theme;
        self
    }

    /// Set the canonical theme mode (single theme authority).
    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.theme_mode = mode;
        self.theme = ThemeTokens::resolve(mode);
    }

    /// Return the effective API key currently entered in the input, if non-empty.
    pub fn effective_api_key(&self) -> Option<String> {
        let text = self.api_key_input.text().trim();
        if !text.is_empty() {
            Some(text.to_string())
        } else {
            None
        }
    }

    /// Human-readable label of where the current credential originated.
    pub fn credential_source_label(&self) -> String {
        let current_key = self.api_key_input.text().trim();
        if current_key.is_empty() {
            return "No credential configured".to_string();
        }

        match &self.initial_credential_source {
            CredentialSource::GlobalFile(path) => {
                format!("M31A user store ({})", path.display())
            }
            CredentialSource::ChannelFile(path) => {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("credentials.json");
                format!("Existing workspace credential store (.m31a/{file_name})")
            }
            CredentialSource::Environment(name) => {
                format!("Environment variable ({name})")
            }
            CredentialSource::Absent => "Entered during onboarding session".to_string(),
        }
    }

    /// Return filtered models matching the current search input.
    pub fn filtered_models(&self) -> Vec<&ModelCandidate> {
        let query = self.model_search_input.text().trim().to_lowercase();
        if query.is_empty() {
            self.catalog.models.iter().collect()
        } else {
            self.catalog
                .models
                .iter()
                .filter(|m| {
                    m.model_id.to_lowercase().contains(&query)
                        || m.display_name
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || m.tier.to_string().contains(&query)
                })
                .collect()
        }
    }

    /// Check eligibility for Primary Reasoning Model role.
    pub fn check_primary_eligibility(candidate: &ModelCandidate) -> Result<(), &'static str> {
        if candidate.availability != ProviderCapabilityStatus::Available {
            return Err("Model is marked unavailable by provider");
        }
        if candidate.context_capacity == 0 || !candidate.is_context_known() {
            return Err("Context window is unknown or unreported by provider");
        }
        if candidate.context_capacity < 4096 {
            return Err("Context window < 4,096 tokens is insufficient for repository planning");
        }
        if !candidate.supports_tools
            || candidate.tool_support == crate::model::router::resolver::CapabilitySupport::Unknown
            || candidate.tool_support
                == crate::model::router::resolver::CapabilitySupport::Unsupported
        {
            return Err("Lacks function/tool calling support required for autonomous execution");
        }
        if candidate.is_embedding() || candidate.model_kind.is_embedding() {
            return Err("Embedding models cannot generate text or code");
        }
        if candidate.is_image_generation() || candidate.model_kind.is_image_generation() {
            return Err("Image generation models cannot generate text or code");
        }
        if candidate.is_safety_guard() || candidate.model_kind.is_safety_guard() {
            return Err("Safety guard models cannot be primary reasoning engines");
        }
        let id_lower = candidate.model_id.to_lowercase();
        if id_lower.contains("embed") {
            return Err("Embedding models cannot generate text or code");
        }
        if id_lower.contains("reward") {
            return Err("Reward models cannot generate conversational solutions");
        }
        if id_lower.contains("guard") && !id_lower.contains("instruct") {
            return Err("Safety guard models cannot be primary reasoning engines");
        }
        Ok(())
    }

    /// Check eligibility for Fast Auxiliary Model role.
    pub fn check_fast_auxiliary_eligibility(
        candidate: &ModelCandidate,
    ) -> Result<(), &'static str> {
        if candidate.availability != ProviderCapabilityStatus::Available {
            return Err("Model is marked unavailable by provider");
        }
        if candidate.context_capacity == 0 || !candidate.is_context_known() {
            return Err("Context window is unknown or unreported by provider");
        }
        if candidate.context_capacity < 4096 {
            return Err("Context window < 4,096 tokens is insufficient");
        }
        if candidate.is_embedding() || candidate.model_kind.is_embedding() {
            return Err("Embedding models cannot generate text");
        }
        if candidate.is_image_generation() || candidate.model_kind.is_image_generation() {
            return Err("Image generation models cannot generate text");
        }
        if candidate.is_safety_guard() || candidate.model_kind.is_safety_guard() {
            return Err("Safety guard models cannot generate text");
        }
        let id_lower = candidate.model_id.to_lowercase();
        if id_lower.contains("embed") {
            return Err("Embedding models cannot generate text");
        }
        if id_lower.contains("reward") {
            return Err("Reward models cannot generate text");
        }
        Ok(())
    }

    /// Synchronously query provider endpoint to discover available models.
    pub fn run_discovery_sync(
        base_url: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<Vec<ModelCandidate>, String> {
        let key = api_key
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("NVIDIA_API_KEY")
                    .or_else(|_| std::env::var("API_KEY_NVIDIA"))
                    .ok()
            });

        let key_str = match key {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Err("Missing API key for model discovery".to_string()),
        };

        let base = base_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        std::thread::spawn(move || -> Result<Vec<ModelCandidate>, String> {
            let provider = NvidiaProvider::new_governed(
                base,
                Some(key_str),
                EndpointTrustSource::BuiltinDefault,
            )
            .map_err(|e| format!("Provider init error: {e}"))?;

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime creation error: {e}"))?;

            rt.block_on(async move {
                provider
                    .discover_models()
                    .await
                    .map_err(|e| format!("Discovery failed: {e}"))
            })
        })
        .join()
        .map_err(|_| "Discovery thread panicked".to_string())?
    }

    /// Perform a truthful, governed verification probe using the minimal chat completion API.
    pub fn run_verification_sync(
        base_url: Option<&str>,
        api_key: &str,
        model: &str,
    ) -> Result<Duration, ModelError> {
        let base = base_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let key_str = api_key.trim().to_string();
        let model_str = model.to_string();

        std::thread::spawn(move || -> Result<Duration, ModelError> {
            let provider = NvidiaProvider::new_governed(
                base,
                Some(key_str),
                EndpointTrustSource::BuiltinDefault,
            )?;

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| ModelError::Network(format!("Runtime creation error: {e}")))?;

            rt.block_on(async move { provider.verify_chat_completion(&model_str).await })
        })
        .join()
        .map_err(|_| ModelError::Network("Verification thread panicked".to_string()))?
    }

    /// Map a runtime ModelError to a typed ProviderVerificationState without leaking secrets.
    pub fn map_model_error_to_verification_state(
        err: ModelError,
        model: &str,
    ) -> ProviderVerificationState {
        match err {
            ModelError::AuthenticationFailed => ProviderVerificationState::AuthenticationFailed {
                status_code: Some(401),
                details:
                    "Authentication failed. NVIDIA NIM rejected the API key or token (HTTP 401)."
                        .to_string(),
            },
            ModelError::Network(msg) => ProviderVerificationState::NetworkError(format!(
                "Unable to reach NVIDIA NIM endpoint: {msg}"
            )),
            ModelError::MissingConfiguration(msg) => {
                if msg.contains("blocked") {
                    ProviderVerificationState::EndpointBlocked(format!(
                        "Provider endpoint blocked by M31A network destination policy: {msg}"
                    ))
                } else {
                    ProviderVerificationState::Misconfigured(msg)
                }
            }
            ModelError::RateLimited { cooldown_secs } => {
                let details = format!("Provider rate limit reached. Retry after {cooldown_secs}s.");
                ProviderVerificationState::RateLimited {
                    retry_after_secs: Some(cooldown_secs),
                    details,
                }
            }
            ModelError::ModelUnavailable(msg) => ProviderVerificationState::ModelUnavailable {
                model: model.to_string(),
                details: format!("Selected model '{model}' is unavailable: {msg}"),
            },
            ModelError::Timeout(msg) => {
                ProviderVerificationState::NetworkError(format!("Connection timed out: {msg}"))
            }
            ModelError::EndpointUnavailable(msg) => {
                ProviderVerificationState::NetworkError(format!("Endpoint unavailable: {msg}"))
            }
            other => ProviderVerificationState::Misconfigured(format!(
                "Verification probe rejected: {other}"
            )),
        }
    }

    /// Execute provider probe for Step 7 with truthful latency and status reporting.
    pub fn verify_provider(&mut self) -> ProviderVerificationState {
        let key = match self.effective_api_key() {
            Some(k) if !k.is_empty() => k,
            _ => {
                let state = ProviderVerificationState::Misconfigured(
                    "Missing API key or token for NVIDIA NIM".to_string(),
                );
                self.verification_state = state.clone();
                self.connection_tested = true;
                self.connection_status = Some("Misconfigured: Missing API key".to_string());
                return state;
            }
        };

        let model_to_verify = self.primary_model_input.text().trim();
        let model = if model_to_verify.is_empty() {
            crate::config::canonical::CANONICAL_DEFAULT_MODEL
        } else {
            model_to_verify
        };

        self.verification_state = ProviderVerificationState::Checking;

        let res = Self::run_verification_sync(self.base_url.as_deref(), &key, model);
        let state = match res {
            Ok(latency) => {
                let endpoint = self.base_url.clone().unwrap_or_else(|| {
                    crate::config::canonical::CANONICAL_NVIDIA_BASE_URL.to_string()
                });
                ProviderVerificationState::Success {
                    latency,
                    model: model.to_string(),
                    endpoint,
                }
            }
            Err(err) => Self::map_model_error_to_verification_state(err, model),
        };

        self.verification_state = state.clone();
        self.connection_tested = true;
        match &state {
            ProviderVerificationState::Success { latency, .. } => {
                self.connection_status = Some(format!("Operational ({latency:?})"));
            }
            ProviderVerificationState::Degraded { latency, reason } => {
                self.connection_status = Some(format!("Degraded ({latency:?}): {reason}"));
            }
            ProviderVerificationState::AuthenticationFailed { .. } => {
                self.connection_status = Some("Authentication failed (HTTP 401)".to_string());
            }
            ProviderVerificationState::NetworkError(msg) => {
                self.connection_status = Some(format!("Network error: {msg}"));
            }
            _ => {
                self.connection_status = Some("Verification failed".to_string());
            }
        }
        state
    }

    /// Perform a connection probe directly on Step 3 (Provider Setup).
    pub fn test_provider_connection_step3(&mut self) -> ProviderVerificationState {
        let key = match self.effective_api_key() {
            Some(k) if !k.is_empty() => k,
            _ => {
                let state = ProviderVerificationState::Misconfigured(
                    "Enter an API key or token before testing connection".to_string(),
                );
                self.step3_connection_state = state.clone();
                return state;
            }
        };

        self.step3_connection_state = ProviderVerificationState::Checking;
        let model = crate::config::canonical::CANONICAL_DEFAULT_MODEL;
        let res = Self::run_verification_sync(self.base_url.as_deref(), &key, model);
        let state = match res {
            Ok(latency) => {
                let endpoint = self.base_url.clone().unwrap_or_else(|| {
                    crate::config::canonical::CANONICAL_NVIDIA_BASE_URL.to_string()
                });
                ProviderVerificationState::Success {
                    latency,
                    model: model.to_string(),
                    endpoint,
                }
            }
            Err(err) => Self::map_model_error_to_verification_state(err, model),
        };
        self.step3_connection_state = state.clone();
        state
    }

    /// Trigger dynamic discovery and update internal catalog, inputs, and cache file.
    pub fn refresh_discovery(&mut self) -> Result<usize, String> {
        let api_key = self.effective_api_key();

        let models = Self::run_discovery_sync(self.base_url.as_deref(), api_key.as_deref())?;
        let count = models.len();
        self.catalog.update_from_provider(
            crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
            models,
        );
        self.catalog.source = CatalogSource::Discovered;
        self.catalog.refresh_state = CatalogRefreshState::DiscoverySuccess;
        self.catalog_verified_live = true;

        // Update default selections from newly discovered catalog
        if let Some(def) = self
            .catalog
            .select_default(Some(crate::config::canonical::CANONICAL_DEFAULT_MODEL))
        {
            self.primary_model_input.set_text(&def.model_id);
        }
        if let Some(fast) = self
            .catalog
            .select_fast_default(Some(self.primary_model_input.text()))
        {
            self.fast_model_input.set_text(&fast.model_id);
        }

        // Save to the canonical global cache (platform cache dir).
        // Legacy workspace cache is no longer written; migration reads it.
        let layout = crate::storage::StorageLayout::for_workspace(&self.workspace_path);
        let cache_path = layout.global_model_catalog_file();
        let _ = self.catalog.save_to_cache_file(&cache_path);

        self.selected_model_index = 0;
        self.model_scroll_offset = 0;
        self.status_message = Some(format!("Discovered {} live models from NVIDIA NIM", count));

        Ok(count)
    }

    /// Persist wizard onboarding choices into canonical stores:
    /// 1. Global user credential store (0600, platform config dir) — NEVER
    ///    workspace-local, never committed.
    /// 2. Configuration TOML — global user config when scope is Global,
    ///    workspace `<ws>/.m31a/config.toml` when scope is Workspace.
    /// 3. Global model-catalog cache (platform cache dir, channel-isolated).
    ///
    /// Provider is fixed to NVIDIA NIM for this release; every other
    /// legitimate setting (models, profile, git, sandbox, approval,
    /// budget incl. Unlimited, concurrency, workflow, theme) is user-chosen
    /// with a shown Recommended default — never silently forced.
    pub fn persist_configuration(&self) -> Result<(), String> {
        let layout = crate::storage::StorageLayout::for_workspace(&self.workspace_path);
        let _ = layout.ensure_global_dirs();
        let _ = layout.ensure_workspace_dir();

        // 1. Credentials ALWAYS go to the global user store (secure, 0600).
        // The workspace must never receive secrets from onboarding.
        let key = self.api_key_input.text().trim();
        if !key.is_empty() {
            let creds_path = layout.global_credentials_file();
            let mut reg = ProviderRegistry::new();
            reg.set_credential(
                crate::config::provider_registry::PRODUCTION_PROVIDER_ID,
                key,
            );
            reg.save_credentials_to_file(&creds_path)
                .map_err(|e| format!("Failed to save credentials: {e}"))?;
        }

        // 2. Configuration honors the user-chosen scope.
        let config_path = match self.config_scope {
            crate::storage::ConfigScope::Global => layout.user_config_file(),
            crate::storage::ConfigScope::Workspace => layout.workspace_config_file(),
        };
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }
        let mut app_config = if config_path.is_file() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read existing configuration: {e}"))?;
            crate::config::schema::parse_and_validate_config(&content)
                .map_err(|e| format!("Existing configuration is invalid: {e}"))?
        } else {
            AppConfig::default()
        };

        // persist the canonical profile id; runtime resolves the same
        // definition via ProfileResolver (round-trip consistency).
        app_config.profile = Some(self.profile.canonical_id().to_string());

        app_config.provider.default =
            crate::config::canonical::CANONICAL_DEFAULT_PROVIDER.to_string();
        let primary = self.primary_model_input.text().trim();
        if !primary.is_empty() {
            app_config.agents.default_model = primary.to_string();
        }
        let fast = self.fast_model_input.text().trim();
        if !fast.is_empty() {
            app_config.agents.fast_auxiliary_model = Some(fast.to_string());
            app_config.agents.fallback_models = vec![fast.to_string()];
        }

        app_config.git.enabled = self.git_enabled;
        app_config.git.auto_commit = self.git_auto_commit;
        app_config.git.push_policy = self.git_push_policy;
        app_config.git.execution_isolation = self.git_execution_isolation;
        app_config.git.branch_prefix = self.git_branch_prefix.clone();

        app_config.policy.interactive_approvals = self.require_approval_for_writes;
        app_config.policy.sandbox_mode = Some(self.sandbox_mode.clone());
        app_config.runtime.sandbox_mode = self.sandbox_mode.clone();

        app_config.workflow.research = self.workflow_research;
        app_config.workflow.auto_advance = self.workflow_auto_advance;
        app_config.runtime.concurrency_limit = self.concurrency_limit;
        // single theme authority: wizard selection -> canonical config.
        app_config.tui.theme = self.theme_mode.to_config_str().to_string();

        if self.unlimited_budget {
            app_config.budget.max_cost_usd = None;
            app_config.budget.max_tokens = None;
            app_config.budget.max_agent_steps = None;
            app_config.budget.max_model_calls = None;
            app_config.budget.max_wall_clock_seconds = None;
            app_config.budget.max_retries = None;
        } else {
            app_config.budget.max_cost_usd = Some(self.max_budget_dollars as f64);
            app_config.budget.max_agent_steps = self.max_agent_steps;
            app_config.budget.max_tokens = self.max_tokens;
            app_config.budget.max_model_calls = self.max_model_calls;
            app_config.budget.max_wall_clock_seconds = self.max_wall_clock_seconds;
            app_config.budget.max_retries = self.max_retries;
        }

        let toml_str = toml::to_string_pretty(&app_config)
            .map_err(|e| format!("Failed to serialize config.toml: {e}"))?;
        std::fs::write(&config_path, toml_str)
            .map_err(|e| format!("Failed to write config.toml: {e}"))?;

        // 3. Model catalog ALWAYS goes to the global platform cache.
        if !self.catalog.is_empty() {
            let cache_path = layout.global_model_catalog_file();
            let _ = self.catalog.save_to_cache_file(&cache_path);
        }

        Ok(())
    }

    /// Refresh doctor diagnostics against workspace.
    pub fn refresh_diagnostics(&mut self) {
        self.probes = self
            .doctor
            .run_all_with_git_enabled(&self.workspace_path, self.git_enabled);
        self.git_info = GitWorkspaceInfo::probe(&self.workspace_path);
    }

    /// Return the active setup step.
    pub fn current_step(&self) -> SetupStep {
        self.current_step
    }

    /// Return workspace directory path.
    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    /// Return the list of diagnostic probes.
    pub fn probes(&self) -> &[DiagnosticProbe] {
        &self.probes
    }

    /// Check if prerequisites for advancing from current step are satisfied.
    ///
    /// Non-negotiable invariant: Step 7 requires actual successful provider verification.
    /// Non-empty key does NOT equal valid key.
    pub fn can_advance(&self) -> bool {
        match self.current_step {
            SetupStep::WorkspaceTrust => self.trust_confirmed,
            SetupStep::DoctorDiagnostics => {
                let has_fatal = self.doctor.has_blocking_failures(&self.probes);
                let has_warn = self.doctor.has_warnings(&self.probes);
                !has_fatal && (!has_warn || self.warnings_acknowledged)
            }
            SetupStep::ProviderSetup => !self.api_key_input.text().trim().is_empty(),
            SetupStep::ModelSetup => !self.primary_model_input.text().trim().is_empty(),
            SetupStep::ProfileSelection => true,
            SetupStep::AutonomySafety => true,
            SetupStep::FinalVerification => self.verification_state.is_success(),
        }
    }

    /// Advance to the next wizard step if valid.
    pub fn advance(&mut self) -> bool {
        if !self.can_advance() {
            return false;
        }

        if self.current_step == SetupStep::ProviderSetup && self.catalog.is_empty() {
            let _ = self.refresh_discovery();
        }

        if let Some(next) = self.current_step.next() {
            self.current_step = next;
            self.status_message = None;
            true
        } else {
            false
        }
    }

    /// Go back to the previous wizard step.
    pub fn back(&mut self) -> bool {
        if let Some(prev) = self.current_step.prev() {
            self.current_step = prev;
            self.status_message = None;
            true
        } else {
            false
        }
    }

    /// Process a keyboard input event and return resulting outcome.
    pub fn handle_key(&mut self, key: KeyEvent) -> WizardOutcome {
        match key.code {
            KeyCode::Esc => {
                if self.current_step == SetupStep::ModelSetup && self.model_focus_search {
                    self.model_focus_search = false;
                    WizardOutcome::None
                } else {
                    WizardOutcome::Cancelled
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                WizardOutcome::Cancelled
            }
            KeyCode::Tab if self.current_step == SetupStep::ModelSetup => {
                self.model_focus_search = !self.model_focus_search;
                WizardOutcome::None
            }
            KeyCode::Char('/')
                if self.current_step == SetupStep::ModelSetup && !self.model_focus_search =>
            {
                self.model_focus_search = true;
                WizardOutcome::None
            }
            KeyCode::Char('r') | KeyCode::Char('R')
                if self.current_step == SetupStep::ModelSetup && !self.model_focus_search =>
            {
                match self.refresh_discovery() {
                    Ok(n) => {
                        self.status_message =
                            Some(format!("Discovered {} live models from NVIDIA NIM", n));
                    }
                    Err(e) => {
                        self.status_message = Some(e);
                    }
                }
                WizardOutcome::None
            }
            KeyCode::Char('1') | KeyCode::Char('p') | KeyCode::Char('P')
                if self.current_step == SetupStep::ModelSetup && !self.model_focus_search =>
            {
                let selected_candidate = self
                    .filtered_models()
                    .get(self.selected_model_index)
                    .map(|c| (*c).clone());
                if let Some(candidate) = selected_candidate {
                    match Self::check_primary_eligibility(&candidate) {
                        Ok(()) => {
                            self.primary_model_input.set_text(&candidate.model_id);
                            self.status_message = Some(format!(
                                "Selected '{}' as Primary Reasoning Model",
                                candidate.model_id
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Ineligible for Primary: {}", e));
                        }
                    }
                } else if self.catalog.is_empty() {
                    self.status_message = Some(
                        "Catalog is empty. Press [R] to discover models from provider.".to_string(),
                    );
                }
                WizardOutcome::None
            }
            KeyCode::Char('2') | KeyCode::Char('a') | KeyCode::Char('A')
                if self.current_step == SetupStep::ModelSetup && !self.model_focus_search =>
            {
                let selected_candidate = self
                    .filtered_models()
                    .get(self.selected_model_index)
                    .map(|c| (*c).clone());
                if let Some(candidate) = selected_candidate {
                    match Self::check_fast_auxiliary_eligibility(&candidate) {
                        Ok(()) => {
                            self.fast_model_input.set_text(&candidate.model_id);
                            self.status_message = Some(format!(
                                "Selected '{}' as Fast Auxiliary Model",
                                candidate.model_id
                            ));
                        }
                        Err(e) => {
                            self.status_message =
                                Some(format!("Ineligible for Fast Auxiliary: {}", e));
                        }
                    }
                } else if self.catalog.is_empty() {
                    self.status_message = Some(
                        "Catalog is empty. Press [R] to discover models from provider.".to_string(),
                    );
                }
                WizardOutcome::None
            }
            KeyCode::PageUp if self.current_step == SetupStep::ModelSetup => {
                self.selected_model_index = self.selected_model_index.saturating_sub(6);
                WizardOutcome::None
            }
            KeyCode::PageDown if self.current_step == SetupStep::ModelSetup => {
                let filtered_len = self.filtered_models().len();
                if filtered_len > 0 {
                    self.selected_model_index =
                        (self.selected_model_index + 6).min(filtered_len - 1);
                }
                WizardOutcome::None
            }
            KeyCode::Home if self.current_step == SetupStep::ModelSetup => {
                self.selected_model_index = 0;
                WizardOutcome::None
            }
            KeyCode::End if self.current_step == SetupStep::ModelSetup => {
                let filtered_len = self.filtered_models().len();
                if filtered_len > 0 {
                    self.selected_model_index = filtered_len - 1;
                }
                WizardOutcome::None
            }
            KeyCode::Char('t') | KeyCode::Char('T')
                if self.current_step == SetupStep::ProviderSetup =>
            {
                let state = self.test_provider_connection_step3();
                match state {
                    ProviderVerificationState::Success { latency, .. } => {
                        self.status_message =
                            Some(format!("Connection probe succeeded ({latency:?})"));
                    }
                    _ => {
                        self.status_message =
                            Some("Connection probe failed. Check API key and network.".to_string());
                    }
                }
                WizardOutcome::None
            }
            KeyCode::Char('t') | KeyCode::Char('T') | KeyCode::Char('v') | KeyCode::Char('V')
                if self.current_step == SetupStep::FinalVerification =>
            {
                let state = self.verify_provider();
                if state.is_success() {
                    self.status_message = Some(
                        "Verification succeeded. Press [Enter] to complete onboarding and launch Cockpit."
                            .to_string(),
                    );
                } else {
                    let err = match &state {
                        ProviderVerificationState::AuthenticationFailed { details, .. } => {
                            format!("Authentication failed: {details}")
                        }
                        ProviderVerificationState::NetworkError(msg) => {
                            format!("Network error: {msg}")
                        }
                        ProviderVerificationState::EndpointBlocked(msg) => {
                            format!("Endpoint blocked: {msg}")
                        }
                        ProviderVerificationState::ModelUnavailable { model, details } => {
                            format!("Model '{model}' unavailable: {details}")
                        }
                        ProviderVerificationState::RateLimited { details, .. } => {
                            format!("Rate limited: {details}")
                        }
                        ProviderVerificationState::Misconfigured(msg) => {
                            format!("Configuration error: {msg}")
                        }
                        _ => "Provider verification probe failed.".to_string(),
                    };
                    self.status_message = Some(err);
                }
                WizardOutcome::None
            }
            KeyCode::Enter => {
                if self.current_step == SetupStep::ModelSetup && self.model_focus_search {
                    self.model_focus_search = false;
                    WizardOutcome::None
                } else if self.current_step == SetupStep::FinalVerification {
                    if self.verification_state.is_success() {
                        WizardOutcome::Completed
                    } else {
                        // Real provider probe on Enter
                        let state = self.verify_provider();
                        if state.is_success() {
                            self.status_message = Some(
                                "Verification succeeded. Press [Enter] to complete onboarding and launch Cockpit."
                                    .to_string(),
                            );
                            WizardOutcome::None
                        } else {
                            let err = match &state {
                                ProviderVerificationState::AuthenticationFailed { details, .. } => {
                                    format!("Authentication failed: {details}")
                                }
                                ProviderVerificationState::NetworkError(msg) => {
                                    format!("Network error: {msg}")
                                }
                                ProviderVerificationState::EndpointBlocked(msg) => {
                                    format!("Endpoint blocked: {msg}")
                                }
                                ProviderVerificationState::ModelUnavailable { model, details } => {
                                    format!("Model '{model}' unavailable: {details}")
                                }
                                ProviderVerificationState::RateLimited { details, .. } => {
                                    format!("Rate limited: {details}")
                                }
                                ProviderVerificationState::Misconfigured(msg) => {
                                    format!("Configuration error: {msg}")
                                }
                                _ => "Provider verification failed. Press Enter to retry or [B] to return to setup."
                                    .to_string(),
                            };
                            self.status_message = Some(err.clone());
                            WizardOutcome::Error(err)
                        }
                    }
                } else if self.can_advance() {
                    let _ = self.advance();
                    WizardOutcome::Advanced(self.current_step)
                } else {
                    let err = match self.current_step {
                        SetupStep::WorkspaceTrust => {
                            "Please confirm repository trust (press Space) before advancing."
                        }
                        SetupStep::DoctorDiagnostics => {
                            if self.doctor.has_blocking_failures(&self.probes) {
                                "Cannot advance: blocking diagnostic failures must be resolved."
                            } else {
                                "Please acknowledge non-critical warnings (press Space) to proceed in degraded mode."
                            }
                        }
                        SetupStep::ProviderSetup => {
                            "Please enter your NVIDIA NIM API Key or token."
                        }
                        SetupStep::ModelSetup => "Primary model name cannot be empty.",
                        SetupStep::FinalVerification => {
                            "Provider verification required before completing setup."
                        }
                        _ => "Requirements for this step are not satisfied.",
                    };
                    self.status_message = Some(err.to_string());
                    WizardOutcome::Error(err.to_string())
                }
            }
            KeyCode::Char('b') | KeyCode::Left if key.modifiers.is_empty() => {
                if self.current_step != SetupStep::ProviderSetup
                    && !(self.current_step == SetupStep::ModelSetup && self.model_focus_search)
                {
                    if self.back() {
                        WizardOutcome::Back(self.current_step)
                    } else {
                        WizardOutcome::None
                    }
                } else {
                    WizardOutcome::None
                }
            }
            KeyCode::Char(' ') => match self.current_step {
                SetupStep::WorkspaceTrust => {
                    self.trust_confirmed = !self.trust_confirmed;
                    WizardOutcome::None
                }
                SetupStep::DoctorDiagnostics => {
                    if self.doctor.has_warnings(&self.probes) {
                        self.warnings_acknowledged = !self.warnings_acknowledged;
                    }
                    WizardOutcome::None
                }
                SetupStep::AutonomySafety => {
                    self.require_approval_for_writes = !self.require_approval_for_writes;
                    WizardOutcome::None
                }
                _ => WizardOutcome::None,
            },
            KeyCode::Char('g') | KeyCode::Char('G')
                if self.current_step == SetupStep::WorkspaceTrust =>
            {
                self.git_enabled = !self.git_enabled;
                self.refresh_diagnostics();
                self.status_message = Some(if self.git_enabled {
                    "Git integration enabled".to_string()
                } else {
                    "Git integration disabled (filesystem-only operation)".to_string()
                });
                WizardOutcome::None
            }
            KeyCode::Char('r') | KeyCode::Char('R')
                if self.current_step == SetupStep::ProfileSelection =>
            {
                self.workflow_research = !self.workflow_research;
                self.status_message = Some(format!(
                    "Workflow Research Phase: {}",
                    if self.workflow_research {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                ));
                WizardOutcome::None
            }
            KeyCode::Char('a') | KeyCode::Char('A')
                if self.current_step == SetupStep::ProfileSelection =>
            {
                self.workflow_auto_advance = !self.workflow_auto_advance;
                self.status_message = Some(format!(
                    "Workflow Auto-Advance: {}",
                    if self.workflow_auto_advance {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                ));
                WizardOutcome::None
            }
            KeyCode::Char('u') | KeyCode::Char('U')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                self.unlimited_budget = !self.unlimited_budget;
                self.status_message = Some(if self.unlimited_budget {
                    "Spend Budget: UNLIMITED (Free Coding)".to_string()
                } else {
                    format!("Spend Budget Ceiling: ${}.00 USD", self.max_budget_dollars)
                });
                WizardOutcome::None
            }
            KeyCode::Char('s') | KeyCode::Char('S')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                self.sandbox_mode = match self.sandbox_mode.as_str() {
                    "standard" => "strict".to_string(),
                    "strict" => "permissive".to_string(),
                    _ => "standard".to_string(),
                };
                self.status_message = Some(format!("Sandbox Mode: {}", self.sandbox_mode));
                WizardOutcome::None
            }
            KeyCode::Char('+') | KeyCode::Char('=')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                if !self.unlimited_budget {
                    self.max_budget_dollars = self.max_budget_dollars.saturating_add(5);
                    self.status_message = Some(format!(
                        "Custom Budget Ceiling: ${}.00 USD",
                        self.max_budget_dollars
                    ));
                } else {
                    self.concurrency_limit = (self.concurrency_limit + 1).min(16);
                    self.status_message =
                        Some(format!("Concurrency limit: {}", self.concurrency_limit));
                }
                WizardOutcome::None
            }
            KeyCode::Char('-') if self.current_step == SetupStep::AutonomySafety => {
                if !self.unlimited_budget {
                    self.max_budget_dollars = self.max_budget_dollars.saturating_sub(5).max(1);
                    self.status_message = Some(format!(
                        "Custom Budget Ceiling: ${}.00 USD",
                        self.max_budget_dollars
                    ));
                } else {
                    self.concurrency_limit = self.concurrency_limit.saturating_sub(1).max(1);
                    self.status_message =
                        Some(format!("Concurrency limit: {}", self.concurrency_limit));
                }
                WizardOutcome::None
            }
            KeyCode::Char('[') if self.current_step == SetupStep::AutonomySafety => {
                self.concurrency_limit = self.concurrency_limit.saturating_sub(1).max(1);
                self.status_message =
                    Some(format!("Concurrency limit: {}", self.concurrency_limit));
                WizardOutcome::None
            }
            KeyCode::Char(']') if self.current_step == SetupStep::AutonomySafety => {
                self.concurrency_limit = (self.concurrency_limit + 1).min(16);
                self.status_message =
                    Some(format!("Concurrency limit: {}", self.concurrency_limit));
                WizardOutcome::None
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                self.git_auto_commit = !self.git_auto_commit;
                self.status_message = Some(format!(
                    "Git Auto-Commit: {}",
                    if self.git_auto_commit {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                ));
                WizardOutcome::None
            }
            KeyCode::Char('p') | KeyCode::Char('P')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                // canonical push policy cycle only (allow/ask/deny).
                use crate::config::schema::GitPushPolicy as Push;
                self.git_push_policy = match self.git_push_policy {
                    Push::Allow => Push::Ask,
                    Push::Ask => Push::Deny,
                    Push::Deny => Push::Allow,
                };
                self.status_message = Some(format!("Git Push Policy: {}", self.git_push_policy));
                WizardOutcome::None
            }
            KeyCode::Char('i') | KeyCode::Char('I')
                if self.current_step == SetupStep::AutonomySafety =>
            {
                // canonical isolation cycle only (required/best_effort).
                use crate::config::schema::GitExecutionIsolation as Iso;
                self.git_execution_isolation = match self.git_execution_isolation {
                    Iso::Required => Iso::BestEffort,
                    Iso::BestEffort => Iso::Required,
                };
                self.status_message = Some(format!(
                    "Git Worktree Isolation: {}",
                    self.git_execution_isolation
                ));
                WizardOutcome::None
            }
            KeyCode::Char('o') | KeyCode::Char('O')
                if matches!(
                    self.current_step,
                    SetupStep::ProfileSelection
                        | SetupStep::AutonomySafety
                        | SetupStep::FinalVerification
                ) =>
            {
                self.toggle_config_scope();
                WizardOutcome::None
            }
            KeyCode::Up => {
                match self.current_step {
                    SetupStep::ProviderSetup => {}
                    SetupStep::ModelSetup => {
                        if self.selected_model_index > 0 {
                            self.selected_model_index -= 1;
                        }
                    }
                    SetupStep::ProfileSelection => {
                        let all = WizardProfile::all();
                        if let Some(idx) = all.iter().position(|p| *p == self.profile) {
                            let prev = (idx + all.len() - 1) % all.len();
                            self.profile = all[prev];
                        }
                    }
                    _ => {}
                }
                WizardOutcome::None
            }
            KeyCode::Down => {
                match self.current_step {
                    SetupStep::ProviderSetup => {}
                    SetupStep::ModelSetup => {
                        let filtered_len = self.filtered_models().len();
                        if self.selected_model_index + 1 < filtered_len {
                            self.selected_model_index += 1;
                        }
                    }
                    SetupStep::ProfileSelection => {
                        let all = WizardProfile::all();
                        if let Some(idx) = all.iter().position(|p| *p == self.profile) {
                            self.profile = all[(idx + 1) % all.len()];
                        }
                    }
                    _ => {}
                }
                WizardOutcome::None
            }
            _ => {
                // Pass text input to active field if on provider or model setup
                match self.current_step {
                    SetupStep::ProviderSetup => {
                        let old_text = self.api_key_input.text().to_string();
                        let _ = self.api_key_input.handle_key(key);
                        if self.api_key_input.text() != old_text {
                            // Invalidate previous verification because credential changed
                            if self.verification_state.is_success() {
                                self.verification_state = ProviderVerificationState::Unverified;
                                self.connection_tested = false;
                                self.connection_status = None;
                            }
                            self.catalog_verified_live = false;
                            self.step3_connection_state = ProviderVerificationState::Unverified;
                        }
                    }
                    SetupStep::ModelSetup => {
                        if self.model_focus_search {
                            let _ = self.model_search_input.handle_key(key);
                            self.selected_model_index = 0;
                            self.model_scroll_offset = 0;
                        } else if self.catalog.is_empty() {
                            let _ = self.primary_model_input.handle_key(key);
                        }
                    }
                    _ => {}
                }
                WizardOutcome::None
            }
        }
    }

    /// Render setup wizard interface to Ratatui buffer.
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Top header and step indicator
                Constraint::Min(12),   // Step body content
                Constraint::Length(3), // Bottom key hints and status
            ])
            .split(area);

        self.render_header(f, chunks[0]);
        self.render_step_body(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    /// Responsive, coherent progress header preserving all 7 steps without clipping.
    fn render_header(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let steps = [
            SetupStep::WorkspaceTrust,
            SetupStep::DoctorDiagnostics,
            SetupStep::ProviderSetup,
            SetupStep::ModelSetup,
            SetupStep::ProfileSelection,
            SetupStep::AutonomySafety,
            SetupStep::FinalVerification,
        ];

        let width = area.width as usize;

        // Line 1: Step indicator spans
        let mut spans = Vec::new();

        if width >= 100 {
            // Full readable names with badges
            for (i, step) in steps.iter().enumerate() {
                let is_current = *step == self.current_step;
                let is_completed = step.step_number() < self.current_step.step_number();

                let (style, label) = if is_current {
                    (
                        tokens.focus.add_modifier(Modifier::BOLD),
                        format!("● {}. {}", step.step_number(), step.title()),
                    )
                } else if is_completed {
                    (
                        tokens.success,
                        format!("✓ {}. {}", step.step_number(), step.title()),
                    )
                } else {
                    (
                        tokens.text_muted,
                        format!("○ {}. {}", step.step_number(), step.title()),
                    )
                };

                spans.push(Span::styled(label, style));
                if i + 1 < steps.len() {
                    spans.push(Span::styled("   ", tokens.text_muted));
                }
            }
        } else if width >= 65 {
            // Short step titles
            let short_names = [
                "Trust", "Doctor", "Provider", "Model", "Profile", "Safety", "Verify",
            ];
            for (i, step) in steps.iter().enumerate() {
                let is_current = *step == self.current_step;
                let is_completed = step.step_number() < self.current_step.step_number();

                let (style, label) = if is_current {
                    (
                        tokens.focus.add_modifier(Modifier::BOLD),
                        format!("● {}", short_names[i]),
                    )
                } else if is_completed {
                    (tokens.success, format!("✓ {}", short_names[i]))
                } else {
                    (tokens.text_muted, format!("○ {}", short_names[i]))
                };

                spans.push(Span::styled(label, style));
                if i + 1 < steps.len() {
                    spans.push(Span::styled("  ", tokens.text_muted));
                }
            }
        } else {
            // Ultra-compact numeric representation
            for (i, step) in steps.iter().enumerate() {
                let is_current = *step == self.current_step;
                let is_completed = step.step_number() < self.current_step.step_number();

                let (style, label) = if is_current {
                    (
                        tokens.focus.add_modifier(Modifier::BOLD),
                        format!("●{}", step.step_number()),
                    )
                } else if is_completed {
                    (tokens.success, format!("✓{}", step.step_number()))
                } else {
                    (tokens.text_muted, format!("○{}", step.step_number()))
                };

                spans.push(Span::styled(label, style));
                if i + 1 < steps.len() {
                    spans.push(Span::styled(" ", tokens.text_muted));
                }
            }
        }

        let header_title = format!(
            " M31A FIRST-RUN SETUP WIZARD (Setup Wizard) · Step {}/7: {} ",
            self.current_step.step_number(),
            self.current_step.title()
        );

        let p = Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_type(BorderType::Plain)
                    .border_style(tokens.border_default)
                    .title(header_title)
                    .title_alignment(Alignment::Center),
            )
            .alignment(Alignment::Center);

        f.render_widget(p, area);
    }

    fn render_step_body(&self, f: &mut Frame, area: Rect) {
        match self.current_step {
            SetupStep::WorkspaceTrust => self.render_workspace_trust(f, area),
            SetupStep::DoctorDiagnostics => self.render_doctor_diagnostics(f, area),
            SetupStep::ProviderSetup => self.render_provider_setup(f, area),
            SetupStep::ModelSetup => self.render_model_setup(f, area),
            SetupStep::ProfileSelection => self.render_profile_selection(f, area),
            SetupStep::AutonomySafety => self.render_autonomy_safety(f, area),
            SetupStep::FinalVerification => self.render_final_verification(f, area),
        }
    }

    fn render_workspace_trust(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 1/7: {} ", SetupStep::WorkspaceTrust.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let trust_box = if self.trust_confirmed { "[✓]" } else { "[ ]" };
        let trust_style = if self.trust_confirmed {
            tokens.success.add_modifier(Modifier::BOLD)
        } else {
            tokens.warning.add_modifier(Modifier::BOLD)
        };

        let git_status_str = if self.git_info.is_git_repo {
            let branch = self.git_info.branch.as_deref().unwrap_or("master");
            let tree_state = if self.git_info.is_clean {
                "Clean working tree"
            } else {
                "Modified working tree"
            };
            format!("Git repository detected (branch: {branch}, {tree_state})")
        } else {
            "No Git repository detected (standard filesystem directory)".to_string()
        };

        if inner.width >= 96 && inner.height >= 14 {
            // Two-column layout for spacious terminals
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(inner);

            let left_lines = vec![
                Line::from(vec![Span::styled(
                    "Workspace Directory",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {}", self.workspace_path.display()),
                    tokens.accent,
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Repository Status",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {git_status_str}"),
                    tokens.text_secondary,
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Source Control Integration",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                if self.git_info.is_git_repo {
                    Line::from(vec![
                        Span::styled("  Use Git integration? ", tokens.text_secondary),
                        Span::styled(
                            if self.git_enabled {
                                " (●) Yes   ( ) No"
                            } else {
                                " ( ) Yes   (●) No"
                            },
                            tokens.focus.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("  [Press 'g' to toggle]", tokens.text_muted),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("  Use M31A without Git? ", tokens.text_secondary),
                        Span::styled(
                            if !self.git_enabled {
                                " (●) Yes (Filesystem only)   ( ) Use Git later"
                            } else {
                                " ( ) Yes   (●) Use Git later"
                            },
                            tokens.focus.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("  [Press 'g' to toggle]", tokens.text_muted),
                    ])
                },
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Trust Decision",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    "  M31 Autonomous requires explicit operator authorization before",
                    tokens.text_secondary,
                )]),
                Line::from(vec![Span::styled(
                    "  executing build tools, test suites, or git operations.",
                    tokens.text_secondary,
                )]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(format!("  {} ", trust_box), trust_style),
                    Span::styled(
                        "I trust this repository, its build scripts, and dependencies (Space to toggle)",
                        if self.trust_confirmed {
                            tokens.text_primary.add_modifier(Modifier::BOLD)
                        } else {
                            tokens.text_primary
                        },
                    ),
                ]),
            ];

            let right_lines = vec![
                Line::from(vec![Span::styled(
                    "M31A will be allowed to:",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(
                        "Inspect source code and project metadata",
                        tokens.text_secondary,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(
                        "Run approved verification and build tools",
                        tokens.text_secondary,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(
                        if self.git_enabled {
                            "Perform governed git worktree operations"
                        } else {
                            "Operate directly on workspace filesystem (Git disabled)"
                        },
                        tokens.text_secondary,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(
                        "Execute authorized model-proposed actions",
                        tokens.text_secondary,
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Protected by Runtime Safeguards:",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::styled("  • Policy Gate: ", tokens.accent),
                    Span::styled(
                        "Zero-bypass policy evaluation on every side effect",
                        tokens.text_muted,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • Container Sandbox: ", tokens.accent),
                    Span::styled(
                        "Filesystem and process boundary isolation",
                        tokens.text_muted,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • Approval Controls: ", tokens.accent),
                    Span::styled(
                        "Human confirmation before sensitive changes",
                        tokens.text_muted,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • Workspace Containment: ", tokens.accent),
                    Span::styled(
                        "Execution bound strictly to project root",
                        tokens.text_muted,
                    ),
                ]),
            ];

            let left_p = Paragraph::new(left_lines).wrap(Wrap { trim: true });
            let right_p = Paragraph::new(right_lines).wrap(Wrap { trim: true });

            f.render_widget(left_p, cols[0]);
            f.render_widget(right_p, cols[1]);
        } else {
            // Compact single-column layout
            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        "Workspace Directory: ",
                        tokens.text_primary.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(self.workspace_path.display().to_string(), tokens.accent),
                ]),
                Line::from(vec![
                    Span::styled(
                        "Repository Status:   ",
                        tokens.text_primary.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(git_status_str, tokens.text_secondary),
                ]),
                if self.git_info.is_git_repo {
                    Line::from(vec![
                        Span::styled(
                            "Source Control:      ",
                            tokens.text_primary.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            if self.git_enabled {
                                "Git Enabled (g to toggle)"
                            } else {
                                "Git Disabled (g to toggle)"
                            },
                            tokens.focus,
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            "Source Control:      ",
                            tokens.text_primary.add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            if !self.git_enabled {
                                "Filesystem Only (g to toggle)"
                            } else {
                                "Git Enabled (g to toggle)"
                            },
                            tokens.focus,
                        ),
                    ])
                },
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Security Notice: ",
                        tokens.warning.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "M31 Autonomous will execute build tools, test suites, and project operations",
                        tokens.text_secondary,
                    ),
                ]),
                Line::from(vec![Span::styled(
                    "within this directory on behalf of autonomous planning objectives.",
                    tokens.text_secondary,
                )]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(format!("  {} ", trust_box), trust_style),
                    Span::styled(
                        "I trust this repository, its build scripts, and dependencies (Space to toggle)",
                        tokens.text_primary,
                    ),
                ]),
            ];

            let p = Paragraph::new(lines).wrap(Wrap { trim: true });
            f.render_widget(p, inner);
        }
    }

    fn render_doctor_diagnostics(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 2/7: {} ",
            SetupStep::DoctorDiagnostics.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let pass_count = self
            .probes
            .iter()
            .filter(|p| p.status == DiagnosticStatus::Pass)
            .count();
        let total_count = self.probes.len();
        let has_warn = self.doctor.has_warnings(&self.probes);
        let has_fatal = self.doctor.has_blocking_failures(&self.probes);

        let mut lines = Vec::new();

        // Summary bar
        let summary_badge = if has_fatal {
            Span::styled(
                format!(" ✗ {}/{} CHECKS PASSED (FATAL) ", pass_count, total_count),
                tokens.error.add_modifier(Modifier::BOLD),
            )
        } else if has_warn {
            Span::styled(
                format!(
                    " ! {}/{} CHECKS PASSED (WARNINGS) ",
                    pass_count, total_count
                ),
                tokens.warning.add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!(" ✓ {}/{} CHECKS PASSED ", pass_count, total_count),
                tokens.success.add_modifier(Modifier::BOLD),
            )
        };

        lines.push(Line::from(vec![
            Span::styled(
                "System Readiness: ",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
            summary_badge,
        ]));
        lines.push(Line::from(""));

        // Diagnostic probe rows
        for probe in &self.probes {
            let (badge_style, symbol) = match probe.status {
                DiagnosticStatus::Pass => (tokens.success.add_modifier(Modifier::BOLD), "✓"),
                DiagnosticStatus::Warn => (tokens.warning.add_modifier(Modifier::BOLD), "!"),
                DiagnosticStatus::Fail => (tokens.error.add_modifier(Modifier::BOLD), "×"),
                DiagnosticStatus::Disabled => (tokens.text_muted.add_modifier(Modifier::BOLD), "-"),
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} {:6} ", symbol, probe.status.badge()),
                    badge_style,
                ),
                Span::styled(
                    format!("{:<26} ", probe.name),
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(&probe.message, tokens.text_secondary),
            ]));

            if let Some(ref rem) = probe.remediation {
                lines.push(Line::from(vec![
                    Span::styled("           ↳ Remediation: ", tokens.text_muted),
                    Span::styled(rem, tokens.accent),
                ]));
            }
        }

        if has_warn {
            let warn_box = if self.warnings_acknowledged {
                "[✓]"
            } else {
                "[ ]"
            };
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", warn_box),
                    tokens.warning.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Acknowledge non-critical warnings and proceed in degraded mode (Space to toggle)",
                    tokens.warning,
                ),
            ]));
        }

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_provider_setup(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 3/7: {} ", SetupStep::ProviderSetup.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines = vec![
            Line::from(vec![Span::styled(
                "Model Provider Engine",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  (●) ", tokens.focus.add_modifier(Modifier::BOLD)),
                Span::styled(
                    "NVIDIA NIM (Inference Microservices)",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(" — Production Provider", tokens.accent),
            ]),
            Line::from(vec![Span::styled(
                "      Dynamic model discovery with OpenAI-compatible API surface.",
                tokens.text_muted,
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Authentication & Credential Authority",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  API Key / Token:  ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("[ {} ]", self.api_key_input.display_text()),
                    if self.api_key_input.text().is_empty() {
                        tokens.text_muted
                    } else {
                        tokens.success.add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(
                    if self.api_key_input.text().is_empty() {
                        "  <type or paste key>"
                    } else {
                        "  (masked)"
                    },
                    tokens.text_muted,
                ),
            ]),
            Line::from(vec![
                Span::styled("  Credential Source: ", tokens.text_primary),
                Span::styled(self.credential_source_label(), tokens.accent),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Connection Probe:    ",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
        ];

        let probe_span = match &self.step3_connection_state {
            ProviderVerificationState::Unverified => Span::styled(
                "  ○ [Not verified] Press [T] to run connection probe",
                tokens.text_muted,
            ),
            ProviderVerificationState::Checking => Span::styled(
                "  • [Checking...] Contacting NVIDIA NIM endpoint...",
                tokens.accent,
            ),
            ProviderVerificationState::Success { latency, .. } => Span::styled(
                format!(
                    "  ✓ [Connected · {latency:?}] Endpoint operational and credential accepted"
                ),
                tokens.success.add_modifier(Modifier::BOLD),
            ),
            ProviderVerificationState::AuthenticationFailed { .. } => Span::styled(
                "  × [Authentication Failed] NVIDIA NIM rejected the credential (HTTP 401)",
                tokens.error.add_modifier(Modifier::BOLD),
            ),
            ProviderVerificationState::NetworkError(msg) => {
                Span::styled(format!("  × [Network Error] {msg}"), tokens.error)
            }
            ProviderVerificationState::EndpointBlocked(msg) => {
                Span::styled(format!("  × [Endpoint Blocked] {msg}"), tokens.error)
            }
            _ => Span::styled("  × [Verification Probe Failed]", tokens.error),
        };
        lines.push(Line::from(vec![probe_span]));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Note: ", tokens.accent),
            Span::styled(
                "Stored credentials are saved exclusively to the channel credential store (.m31a) with 0600 permissions.",
                tokens.text_muted,
            ),
        ]));

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_model_setup(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 4/7: {} ", SetupStep::ModelSetup.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if inner.height < 10 || inner.width < 50 {
            self.render_model_setup_compact(f, inner);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search & status bar
                Constraint::Min(6),    // Main catalog list & model inspection
                Constraint::Length(5), // Role assignments & shortcuts
            ])
            .split(inner);

        // 1. Search Bar & Catalog Status
        let filtered = self.filtered_models();
        let prov_display = "NVIDIA NIM";

        let catalog_status_str = if self.catalog_verified_live {
            "LIVE"
        } else if !self.catalog.is_empty() {
            "CACHED · Not verified against current credentials"
        } else {
            "NOT VERIFIED"
        };

        let search_border_style = if self.model_focus_search {
            tokens.focus.add_modifier(Modifier::BOLD)
        } else {
            tokens.border_default
        };

        let search_title = format!(
            " Provider: {} | Catalog: {} discovered ({}) | Showing: {}/{} ",
            prov_display,
            self.catalog.len(),
            catalog_status_str,
            filtered.len(),
            self.catalog.len()
        );

        let search_block = Block::default()
            .borders(Borders::NONE)
            .title(search_title)
            .border_style(search_border_style);

        let display_buf = self.model_search_input.display_text();
        let search_text = if self.model_search_input.text().is_empty() && !self.model_focus_search {
            " Type to search/filter models (press Tab or / to focus)..."
        } else {
            &display_buf
        };

        let search_p = Paragraph::new(Line::from(vec![
            Span::styled(
                " Search: ",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                search_text,
                if self.model_search_input.text().is_empty() && !self.model_focus_search {
                    tokens.text_muted
                } else {
                    tokens.accent
                },
            ),
        ]))
        .block(search_block);
        f.render_widget(search_p, chunks[0]);

        // 2. Middle: Left Catalog List (58%) & Right Details (42%)
        let middle_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(chunks[1]);

        // 2a. Catalog List
        let list_block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Available Models ({}) ", filtered.len()))
            .border_style(if !self.model_focus_search {
                tokens.focus
            } else {
                tokens.border_default
            });

        let list_inner = list_block.inner(middle_chunks[0]);
        f.render_widget(list_block, middle_chunks[0]);

        let visible_height = list_inner.height as usize;
        let selected_idx = self
            .selected_model_index
            .min(filtered.len().saturating_sub(1));

        let scroll_offset = if selected_idx < self.model_scroll_offset {
            selected_idx
        } else if selected_idx >= self.model_scroll_offset + visible_height {
            selected_idx.saturating_sub(visible_height.saturating_sub(1))
        } else {
            self.model_scroll_offset
        };

        let mut list_lines = Vec::new();
        if filtered.is_empty() {
            if self.catalog.is_empty() {
                list_lines.push(Line::from(Span::styled(
                    " No models discovered yet.",
                    tokens.warning,
                )));
                list_lines.push(Line::from(Span::styled(
                    " Press [R] to discover live models from NVIDIA NIM,",
                    tokens.text_muted,
                )));
                list_lines.push(Line::from(Span::styled(
                    " or configure API key in Step 3.",
                    tokens.text_muted,
                )));
            } else {
                list_lines.push(Line::from(Span::styled(
                    " No models match the search query.",
                    tokens.warning,
                )));
            }
        } else {
            let primary_id = self.primary_model_input.text();
            let fast_id = self.fast_model_input.text();

            for (i, candidate) in filtered
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(visible_height)
            {
                let is_current = i == selected_idx;
                let is_primary = candidate.model_id == primary_id;
                let is_fast = candidate.model_id == fast_id;

                let role_tag = if is_primary && is_fast {
                    " [PRI+AUX]"
                } else if is_primary {
                    " [PRIMARY]"
                } else if is_fast {
                    " [FAST]"
                } else {
                    ""
                };

                let tier_short = match candidate.tier {
                    ModelTier::Reasoning => "Rsn",
                    ModelTier::Standard => "Std",
                    ModelTier::Fast => "Fast",
                };

                let ctx_short = if candidate.context_capacity >= 1000 {
                    format!("{}k", candidate.context_capacity / 1000)
                } else if candidate.context_capacity > 0 {
                    format!("{}", candidate.context_capacity)
                } else {
                    "unk".to_string()
                };

                let tools_symbol = match candidate.tool_support {
                    crate::model::router::resolver::CapabilitySupport::Supported => "✓",
                    crate::model::router::resolver::CapabilitySupport::Unsupported => "✗",
                    crate::model::router::resolver::CapabilitySupport::Unknown => "?",
                };
                let prefix = if is_current { "❯ " } else { "  " };

                let line_style = if is_current {
                    tokens.selection.add_modifier(Modifier::BOLD)
                } else if is_primary {
                    tokens.focus
                } else if is_fast {
                    tokens.success
                } else {
                    tokens.text_secondary
                };

                let id_max_len = (list_inner.width as usize).saturating_sub(30);
                let truncated_id = if candidate.model_id.len() > id_max_len && id_max_len > 3 {
                    format!("{}…", &candidate.model_id[..id_max_len.saturating_sub(1)])
                } else {
                    candidate.model_id.clone()
                };

                let line = Line::from(vec![
                    Span::styled(prefix, line_style),
                    Span::styled(
                        format!("{:<width$}", truncated_id, width = id_max_len),
                        line_style,
                    ),
                    Span::styled(
                        format!(" {:<4} {:>4}  {} ", tier_short, ctx_short, tools_symbol),
                        line_style,
                    ),
                    Span::styled(
                        role_tag,
                        if is_primary {
                            tokens.focus.add_modifier(Modifier::BOLD)
                        } else {
                            tokens.success.add_modifier(Modifier::BOLD)
                        },
                    ),
                ]);
                list_lines.push(line);
            }
        }

        let p_list = Paragraph::new(list_lines);
        f.render_widget(p_list, list_inner);

        // 2b. Model Details & Eligibility Panel
        let details_block = Block::default()
            .borders(Borders::NONE)
            .title(" Model Metadata & Eligibility ")
            .border_style(tokens.border_default);
        let details_inner = details_block.inner(middle_chunks[1]);
        f.render_widget(details_block, middle_chunks[1]);

        let mut detail_lines = Vec::new();
        if let Some(candidate) = filtered.get(selected_idx) {
            detail_lines.push(Line::from(vec![
                Span::styled(
                    "Model ID: ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(&candidate.model_id, tokens.accent),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Provider: ", tokens.text_primary),
                Span::styled(
                    format!("{} ({})", candidate.provider, candidate.source),
                    tokens.text_secondary,
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Source:   ", tokens.text_primary),
                Span::styled(
                    candidate.context_provenance().unwrap_or("unknown"),
                    tokens.accent,
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Kind:     ", tokens.text_primary),
                Span::styled(format!("{}", candidate.model_kind), tokens.text_secondary),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Tier:     ", tokens.text_primary),
                Span::styled(format!("{}", candidate.tier), tokens.accent),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Context:  ", tokens.text_primary),
                Span::styled(
                    if candidate.context_capacity > 0 {
                        format!("{} tokens", candidate.context_capacity)
                    } else {
                        "Unknown (unreported by provider)".to_string()
                    },
                    if candidate.context_capacity >= 4096 && candidate.is_context_known() {
                        tokens.success
                    } else {
                        tokens.error
                    },
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Tools:    ", tokens.text_primary),
                match candidate.tool_support {
                    crate::model::router::resolver::CapabilitySupport::Supported => {
                        Span::styled("Supported (✓)", tokens.success)
                    }
                    crate::model::router::resolver::CapabilitySupport::Unsupported => {
                        Span::styled("Unsupported (✗)", tokens.error)
                    }
                    crate::model::router::resolver::CapabilitySupport::Unknown => {
                        Span::styled("Unknown (?)", tokens.warning)
                    }
                },
            ]));
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                "Role Eligibility Evaluation:",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )));

            let primary_res = Self::check_primary_eligibility(candidate);
            let fast_res = Self::check_fast_auxiliary_eligibility(candidate);

            detail_lines.push(Line::from(vec![
                Span::styled("  Primary: ", tokens.text_primary),
                match primary_res {
                    Ok(()) => Span::styled("✓ Eligible (Tool calling & context)", tokens.success),
                    Err(e) => Span::styled(format!("✗ Ineligible: {}", e), tokens.error),
                },
            ]));

            detail_lines.push(Line::from(vec![
                Span::styled("  Fast:    ", tokens.text_primary),
                match fast_res {
                    Ok(()) => {
                        Span::styled("✓ Eligible (Auxiliary / summarization)", tokens.success)
                    }
                    Err(e) => Span::styled(format!("✗ Ineligible: {}", e), tokens.warning),
                },
            ]));
        } else {
            detail_lines.push(Line::from(Span::styled(
                "Select a model to view capabilities & role eligibility.",
                tokens.text_muted,
            )));
        }

        let p_details = Paragraph::new(detail_lines).wrap(Wrap { trim: true });
        f.render_widget(p_details, details_inner);

        // 3. Bottom Roles & Actions
        let roles_block = Block::default()
            .borders(Borders::NONE)
            .title(" Active Role Assignments ")
            .border_style(tokens.border_default);
        let roles_inner = roles_block.inner(chunks[2]);
        f.render_widget(roles_block, chunks[2]);

        let roles_lines = vec![
            Line::from(vec![
                Span::styled(
                    "Primary Reasoning: ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.primary_model_input.display_text(),
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  (DAG planning, code synthesis, architecture)",
                    tokens.text_muted,
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Fast Auxiliary:    ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.fast_model_input.display_text(),
                    tokens.success.add_modifier(Modifier::BOLD),
                ),
                Span::styled("  (Linting, small tools, summaries)", tokens.text_muted),
            ]),
            Line::from(vec![
                Span::styled("[1/p] ", tokens.focus.add_modifier(Modifier::BOLD)),
                Span::styled("Set Primary  ", tokens.text_secondary),
                Span::styled("[2/a] ", tokens.success.add_modifier(Modifier::BOLD)),
                Span::styled("Set Fast  ", tokens.text_secondary),
                Span::styled("[Tab//] ", tokens.accent.add_modifier(Modifier::BOLD)),
                Span::styled("Search/List  ", tokens.text_secondary),
                Span::styled("[R] ", tokens.accent.add_modifier(Modifier::BOLD)),
                Span::styled("Refresh Discovery  ", tokens.text_secondary),
                Span::styled("[Enter] ", tokens.text_primary.add_modifier(Modifier::BOLD)),
                Span::styled("Confirm & Advance", tokens.text_secondary),
            ]),
        ];

        let p_roles = Paragraph::new(roles_lines);
        f.render_widget(p_roles, roles_inner);
    }

    fn render_model_setup_compact(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Primary Reasoning Model: ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(self.primary_model_input.display_text(), tokens.focus),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Fast Auxiliary Model:    ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(self.fast_model_input.display_text(), tokens.success),
            ]),
        ];
        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }

    fn render_profile_selection(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 5/7: {} ",
            SetupStep::ProfileSelection.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let profiles = WizardProfile::all();

        let mut lines = vec![
            Line::from(vec![Span::styled(
                "Execution Profile (Use Up/Down arrow keys to select):",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        for p in profiles {
            let is_sel = *p == self.profile;
            let (radio, style) = if is_sel {
                ("  [●] ", tokens.focus.add_modifier(Modifier::BOLD))
            } else {
                ("  [ ] ", tokens.text_muted)
            };

            lines.push(Line::from(vec![
                Span::styled(radio, style),
                Span::styled(
                    format!("{:<16} ", p.name().to_uppercase()),
                    if is_sel {
                        tokens.focus.add_modifier(Modifier::BOLD)
                    } else {
                        tokens.text_primary
                    },
                ),
                Span::styled(p.description(), tokens.text_secondary),
            ]));

            if is_sel {
                lines.push(Line::from(vec![
                    Span::styled("       ↳ ", tokens.accent),
                    Span::styled(p.recommendation(), tokens.accent),
                ]));
            }
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![Span::styled(
            "Workflow Pipeline Controls:",
            tokens.text_primary.add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                if self.workflow_research {
                    "  [✓] "
                } else {
                    "  [ ] "
                },
                tokens.focus.add_modifier(Modifier::BOLD),
            ),
            Span::styled("Research & Context Discovery Phase  ", tokens.text_primary),
            Span::styled("[Press 'r' to toggle]", tokens.text_muted),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                if self.workflow_auto_advance {
                    "  [✓] "
                } else {
                    "  [ ] "
                },
                tokens.focus.add_modifier(Modifier::BOLD),
            ),
            Span::styled("Workflow Stage Auto-Advance       ", tokens.text_primary),
            Span::styled("[Press 'a' to toggle]", tokens.text_muted),
        ]));

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_autonomy_safety(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 6/7: {} ", SetupStep::AutonomySafety.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let apprv_box = if self.require_approval_for_writes {
            "[✓]"
        } else {
            "[ ]"
        };

        let budget_str = if self.unlimited_budget {
            format!(
                "UNLIMITED (Free Coding · custom ceiling: ${}.00 USD if enabled)",
                self.max_budget_dollars
            )
        } else {
            format!("${}.00 USD Ceiling", self.max_budget_dollars)
        };

        let lines = vec![
            Line::from(vec![Span::styled(
                "Execution Policy Guardrails",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("  {} ", apprv_box),
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Require human authorization for destructive file modifications and shell executions (Space to toggle)",
                    tokens.text_primary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  Sandbox Mode: ", tokens.text_secondary),
                Span::styled(
                    &self.sandbox_mode,
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  [Press 's' to cycle: standard / strict / permissive]",
                    tokens.text_muted,
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Resource Ceilings (Never Arbitrarily Stop Coding)",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  Spend Budget: ", tokens.text_secondary),
                Span::styled(
                    budget_str,
                    if self.unlimited_budget {
                        tokens.success.add_modifier(Modifier::BOLD)
                    } else {
                        tokens.warning.add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(
                    "  [Press 'u' to toggle Unlimited vs Custom, +/- to adjust]",
                    tokens.text_muted,
                ),
            ]),
            Line::from(vec![
                Span::styled("  Concurrency:  ", tokens.text_secondary),
                Span::styled(
                    format!("{} concurrent tasks", self.concurrency_limit),
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
                Span::styled("  [Press [ / ] to adjust]", tokens.text_muted),
            ]),
            Line::from(vec![
                Span::styled("  Step / Token / Call Ceilings: ", tokens.text_secondary),
                Span::styled(
                    "Unlimited by default (bounded by DAG completion & verification)",
                    tokens.text_muted,
                ),
            ]),
            Line::from(""),
            if self.git_enabled {
                Line::from(vec![
                    Span::styled(
                        "Git Governance: ",
                        tokens.text_primary.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            "Auto-commit: {} [c]  |  Push: {} [p]  |  Isolation: {} [i]",
                            if self.git_auto_commit { "On" } else { "Off" },
                            self.git_push_policy,
                            self.git_execution_isolation,
                        ),
                        tokens.accent,
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        "Git Governance: ",
                        tokens.text_primary.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Disabled by user preference (no worktrees or commits)",
                        tokens.text_muted,
                    ),
                ])
            },
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Security Model Pipeline: ",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Policy Gate → Human Approval → Sandbox → Governed Execution (Zero exceptions)",
                    tokens.text_muted,
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Configuration Scope (where settings live)",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  ", tokens.text_secondary),
                Span::styled(
                    match self.config_scope {
                        crate::storage::ConfigScope::Workspace => {
                            "(●) This workspace   ( ) All my M31A projects"
                        }
                        crate::storage::ConfigScope::Global => {
                            "( ) This workspace   (●) All my M31A projects"
                        }
                    },
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
                Span::styled("  [Press 'o' to toggle]", tokens.text_muted),
            ]),
            Line::from(vec![Span::styled(
                format!("  {}", self.config_scope_label()),
                tokens.text_muted,
            )]),
            Line::from(vec![Span::styled(
                "  Credentials → M31A user store only (never committed). Cache → platform cache.",
                tokens.text_muted,
            )]),
        ];

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_final_verification(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 7/7: {} ",
            SetupStep::FinalVerification.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let endpoint_str = self
            .base_url
            .as_deref()
            .unwrap_or(crate::config::canonical::CANONICAL_NVIDIA_BASE_URL);

        let git_summary = if self.git_enabled {
            format!(
                "Enabled (commit: {}, push: {}, iso: {})",
                if self.git_auto_commit {
                    "auto"
                } else {
                    "manual"
                },
                self.git_push_policy,
                self.git_execution_isolation,
            )
        } else {
            "Disabled by user preference (filesystem only)".to_string()
        };

        let budget_summary = if self.unlimited_budget {
            "UNLIMITED (Free Coding - No arbitrary ceilings)".to_string()
        } else {
            format!("${}.00 USD ceiling", self.max_budget_dollars)
        };

        let mut left_lines = vec![
            Line::from(vec![Span::styled(
                "Onboarding Configuration Summary",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ✓ Workspace:     ", tokens.success),
                Span::styled(
                    self.workspace_path.display().to_string(),
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Git Policy:    ", tokens.success),
                Span::styled(git_summary, tokens.text_secondary),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Provider:      ", tokens.success),
                Span::styled("NVIDIA NIM (Production)", tokens.text_secondary),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Primary Model: ", tokens.success),
                Span::styled(
                    self.primary_model_input.text(),
                    tokens.focus.add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Fast Model:    ", tokens.success),
                Span::styled(self.fast_model_input.text(), tokens.success),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Catalog:       ", tokens.success),
                Span::styled(
                    format!("{} models discovered", self.catalog.len()),
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Profile:       ", tokens.success),
                Span::styled(self.profile.name(), tokens.text_secondary),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Safety Policy: ", tokens.success),
                Span::styled(
                    format!(
                        "{}, sandbox: {}",
                        if self.require_approval_for_writes {
                            "Human approval required"
                        } else {
                            "Autonomous write approval"
                        },
                        self.sandbox_mode
                    ),
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Spend Budget:  ", tokens.success),
                Span::styled(budget_summary, tokens.text_secondary),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Concurrency:   ", tokens.success),
                Span::styled(
                    format!("{} parallel tasks", self.concurrency_limit),
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Workflow:      ", tokens.success),
                Span::styled(
                    format!(
                        "Research: {}, Auto-Advance: {}",
                        if self.workflow_research { "On" } else { "Off" },
                        if self.workflow_auto_advance {
                            "On"
                        } else {
                            "Off"
                        },
                    ),
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![
                Span::styled("  ✓ Scope:         ", tokens.success),
                Span::styled(
                    match self.config_scope {
                        crate::storage::ConfigScope::Workspace => "This workspace only [o]",
                        crate::storage::ConfigScope::Global => "All my M31A projects [o]",
                    },
                    tokens.text_secondary,
                ),
            ]),
            Line::from(vec![Span::styled(
                format!("  {}", self.config_scope_label()),
                tokens.text_muted,
            )]),
        ];

        let mut right_lines = vec![
            Line::from(vec![Span::styled(
                "Provider Authentication & Readiness",
                tokens.text_primary.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Endpoint:          ", tokens.text_primary),
                Span::styled(endpoint_str, tokens.accent),
            ]),
            Line::from(vec![
                Span::styled("  Credential Source: ", tokens.text_primary),
                Span::styled(self.credential_source_label(), tokens.text_secondary),
            ]),
            Line::from(vec![
                Span::styled("  Model Probed:      ", tokens.text_primary),
                Span::styled(self.primary_model_input.text(), tokens.text_secondary),
            ]),
            Line::from(""),
        ];

        match &self.verification_state {
            ProviderVerificationState::Unverified => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled("○ UNVERIFIED", tokens.warning.add_modifier(Modifier::BOLD)),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    "  A live verification probe against NVIDIA NIM is required before",
                    tokens.text_muted,
                )]));
                right_lines.push(Line::from(vec![Span::styled(
                    "  onboarding can be completed.",
                    tokens.text_muted,
                )]));
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(vec![
                    Span::styled(
                        "  [Enter] or [T] ",
                        tokens.focus.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Run live verification probe now", tokens.text_primary),
                ]));
            }
            ProviderVerificationState::Checking => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled("• CHECKING...", tokens.accent.add_modifier(Modifier::BOLD)),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    "  Sending authenticated minimal chat request to NVIDIA NIM...",
                    tokens.text_muted,
                )]));
            }
            ProviderVerificationState::Success { latency, model, .. } => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled("✓ OPERATIONAL", tokens.success.add_modifier(Modifier::BOLD)),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled("NVIDIA NIM credential accepted", tokens.text_secondary),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled("Endpoint reachable", tokens.text_secondary),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(format!("Model '{model}' accepted"), tokens.text_secondary),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  ✓ ", tokens.success),
                    Span::styled(
                        format!("Truthful round-trip latency: {latency:?}"),
                        tokens.success,
                    ),
                ]));
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(vec![
                    Span::styled(
                        "  Press [Enter] ",
                        tokens.text_primary.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "to complete onboarding and launch the M31A Cockpit.",
                        tokens.text_secondary,
                    ),
                ]));
            }
            ProviderVerificationState::AuthenticationFailed { details, .. } => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled(
                        "✗ AUTHENTICATION FAILED",
                        tokens.error.add_modifier(Modifier::BOLD),
                    ),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  {details}"),
                    tokens.error,
                )]));
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(vec![Span::styled(
                    "  Next steps:",
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                )]));
                right_lines.push(Line::from(vec![Span::styled(
                    "  • Press [B] to return to Provider Setup and replace the credential.",
                    tokens.text_secondary,
                )]));
                right_lines.push(Line::from(vec![Span::styled(
                    "  • Press [Enter] or [T] to retry verification probe.",
                    tokens.text_secondary,
                )]));
            }
            ProviderVerificationState::NetworkError(msg) => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled("✗ NETWORK ERROR", tokens.error.add_modifier(Modifier::BOLD)),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  {msg}"),
                    tokens.error,
                )]));
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(vec![Span::styled(
                    "  Check internet connectivity, firewall, or DNS resolution.",
                    tokens.text_muted,
                )]));
            }
            ProviderVerificationState::EndpointBlocked(msg) => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled(
                        "✗ ENDPOINT BLOCKED",
                        tokens.error.add_modifier(Modifier::BOLD),
                    ),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  {msg}"),
                    tokens.error,
                )]));
            }
            ProviderVerificationState::ModelUnavailable { model, details } => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled(
                        "✗ MODEL UNAVAILABLE",
                        tokens.error.add_modifier(Modifier::BOLD),
                    ),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  Model '{model}': {details}"),
                    tokens.error,
                )]));
                right_lines.push(Line::from(""));
                right_lines.push(Line::from(vec![Span::styled(
                    "  Press [B] to return to Model Setup and choose an available model.",
                    tokens.text_muted,
                )]));
            }
            ProviderVerificationState::RateLimited { details, .. } => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled(
                        "! RATE LIMITED",
                        tokens.warning.add_modifier(Modifier::BOLD),
                    ),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  {details}"),
                    tokens.warning,
                )]));
            }
            ProviderVerificationState::Misconfigured(msg) => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled("✗ MISCONFIGURED", tokens.error.add_modifier(Modifier::BOLD)),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  {msg}"),
                    tokens.error,
                )]));
            }
            ProviderVerificationState::Degraded { latency, reason } => {
                right_lines.push(Line::from(vec![
                    Span::styled("  Status: ", tokens.text_primary),
                    Span::styled(
                        "! OPERATIONAL (DEGRADED)",
                        tokens.warning.add_modifier(Modifier::BOLD),
                    ),
                ]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  Probe latency: {latency:?}"),
                    tokens.warning,
                )]));
                right_lines.push(Line::from(vec![Span::styled(
                    format!("  Notice: {reason}"),
                    tokens.text_muted,
                )]));
            }
        }

        if inner.width >= 96 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
                .split(inner);

            let left_p = Paragraph::new(left_lines).wrap(Wrap { trim: true });
            let right_p = Paragraph::new(right_lines).wrap(Wrap { trim: true });

            f.render_widget(left_p, cols[0]);
            f.render_widget(right_p, cols[1]);
        } else {
            left_lines.push(Line::from(""));
            left_lines.extend(right_lines);
            let p = Paragraph::new(left_lines).wrap(Wrap { trim: true });
            f.render_widget(p, inner);
        }
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let tokens = &self.theme;

        let mut spans = vec![
            Span::styled("[Enter] ", tokens.focus.add_modifier(Modifier::BOLD)),
            Span::styled(
                if self.current_step == SetupStep::FinalVerification {
                    if self.verification_state.is_success() {
                        "Complete & Launch Cockpit  "
                    } else {
                        "Verify Provider  "
                    }
                } else {
                    "Next Step  "
                },
                tokens.text_primary,
            ),
            Span::styled("[B] ", tokens.accent.add_modifier(Modifier::BOLD)),
            Span::styled("Back  ", tokens.text_secondary),
            Span::styled("[Esc] ", tokens.accent.add_modifier(Modifier::BOLD)),
            Span::styled("Cancel Setup", tokens.text_secondary),
        ];

        match self.current_step {
            SetupStep::WorkspaceTrust => {
                spans.push(Span::styled(
                    "  [Space] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Trust  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[G] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Toggle Git", tokens.text_secondary));
            }
            SetupStep::DoctorDiagnostics => {
                spans.push(Span::styled(
                    "  [Space] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Ack Warnings", tokens.text_secondary));
            }
            SetupStep::ProviderSetup => {
                spans.push(Span::styled(
                    "  [T] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Test Connection", tokens.text_secondary));
            }
            SetupStep::ModelSetup => {
                spans.push(Span::styled(
                    "  [R] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Refresh  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[/] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Search", tokens.text_secondary));
            }
            SetupStep::ProfileSelection => {
                spans.push(Span::styled(
                    "  [↑/↓] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Profile  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[R] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Research  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[A] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Auto-Advance", tokens.text_secondary));
            }
            SetupStep::AutonomySafety => {
                spans.push(Span::styled(
                    "  [Space] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Approvals  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[U] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Unlimited/Custom  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[+/-] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Budget/Tasks  ", tokens.text_secondary));
                spans.push(Span::styled(
                    "[S] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Sandbox", tokens.text_secondary));
            }
            SetupStep::FinalVerification => {
                spans.push(Span::styled(
                    "  [T] ",
                    tokens.accent.add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("Probe", tokens.text_secondary));
            }
        }

        if let Some(ref msg) = self.status_message {
            spans.push(Span::styled("  |  ", tokens.text_muted));
            spans.push(Span::styled(msg, tokens.error.add_modifier(Modifier::BOLD)));
        }

        let p = Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_type(BorderType::Plain)
                    .border_style(tokens.border_default),
            )
            .alignment(Alignment::Center);

        f.render_widget(p, area);
    }
}
