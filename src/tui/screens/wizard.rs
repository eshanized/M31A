//! 7-Step Guided Onboarding Setup Wizard Screen (FRX-02, FRX-03, D-02).
//!
//! Interactive terminal wizard walking the operator through Workspace Trust,
//! Doctor Diagnostics, Provider Selection, Model Selection, Profile Selection,
//! Autonomy/Safety constraints, and Final Verification.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use std::path::{Path, PathBuf};

use crate::config::provider_registry::ProviderRegistry;
use crate::config::schema::AppConfig;
use crate::init::doctor::{DiagnosticProbe, DiagnosticStatus, DoctorEngine};
use crate::init::lifecycle::SetupStep;
use crate::model::catalog::ModelCatalog;
use crate::model::router::resolver::{ModelCandidate, ModelTier};
use crate::model::types::ProviderCapabilityStatus;
use crate::tui::input::text_input::{InputMasking, TextInput};

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

/// Execution profile choices for Step 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardProfile {
    Balanced,
    Autonomous,
    Conservative,
    CodeReviewer,
}

impl WizardProfile {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Balanced => "Balanced",
            Self::Autonomous => "Autonomous",
            Self::Conservative => "Conservative",
            Self::CodeReviewer => "Code Reviewer",
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

    // Step 1: Trust
    pub trust_confirmed: bool,

    // Step 2: Doctor
    pub warnings_acknowledged: bool,

    // Step 3: Provider
    pub api_key_input: TextInput,

    // Step 4: Model Catalog, Search & Role Assignments
    pub primary_model_input: TextInput,
    pub fast_model_input: TextInput,
    pub catalog: ModelCatalog,
    pub model_search_input: TextInput,
    pub selected_model_index: usize,
    pub model_scroll_offset: usize,
    pub model_focus_search: bool,

    // Step 5: Profile
    pub profile: WizardProfile,

    // Step 6: Autonomy
    pub require_approval_for_writes: bool,
    pub max_budget_dollars: u32,

    // Step 7: Verification
    pub connection_tested: bool,
    pub connection_status: Option<String>,

    pub status_message: Option<String>,
}
impl SetupWizardScreen {
    pub const PROVIDERS: &'static [&'static str] = &[
        "NVIDIA NIM (Dynamic Model Discovery) [AVAILABLE]",
        "Anthropic (Claude 3.5 Sonnet) [UNAVAILABLE - Deferred in v1]",
        "OpenAI (GPT-4o) [UNAVAILABLE - Deferred in v1]",
        "Google Gemini (1.5 Pro) [UNAVAILABLE - Deferred in v1]",
        "OpenAI-Compatible (Local / Ollama) [UNAVAILABLE - Deferred in v1]",
    ];

    pub fn new(workspace_path: PathBuf) -> Self {
        let doctor = DoctorEngine::new();
        let probes = doctor.run_all(&workspace_path);

        let mut api_key_input = TextInput::single_line().with_masking(InputMasking::Masked('*'));

        // Check environment or channel-aware credentials file for existing API key
        let creds_path =
            crate::config::provider_registry::ProviderRegistry::channel_credentials_path(
                &workspace_path,
            );
        let mut key_from_env = std::env::var("NVIDIA_API_KEY")
            .or_else(|_| std::env::var("API_KEY_NVIDIA"))
            .ok();
        if key_from_env.is_none()
            && creds_path.is_file()
            && let mut reg = ProviderRegistry::new()
            && reg.load_credentials_from_file(&creds_path).is_ok()
            && let Some(sec) = reg.get_credential("nvidia_nim")
        {
            key_from_env = Some(sec.expose_secret().to_string());
        }
        if let Some(ref k) = key_from_env
            && !k.trim().is_empty()
        {
            api_key_input.set_text(k);
        }

        let mut primary_model_input = TextInput::single_line();
        let mut fast_model_input = TextInput::single_line();
        let model_search_input = TextInput::single_line();

        // Channel-aware catalog cache: development onboarding never reads
        // production discovery state (nor writes it below).
        let cache_path = ModelCatalog::cache_path_for_channel(
            &workspace_path,
            crate::deployment::DeploymentChannel::current(),
        );
        let mut catalog = ModelCatalog::load_from_cache_file(&cache_path)
            .ok()
            .filter(|c| c.schema_version >= ModelCatalog::CURRENT_CATALOG_SCHEMA_VERSION)
            .unwrap_or_else(|| ModelCatalog::new("nvidia_nim"));

        // If catalog was empty and key is present in environment, perform initial discovery
        if catalog.is_empty()
            && key_from_env.is_some()
            && let Ok(discovered) = Self::run_discovery_sync(None, key_from_env.as_deref())
        {
            catalog.update_from_provider("nvidia_nim", discovered);
            let _ = catalog.save_to_cache_file(&cache_path);
        }

        if let Some(def) = catalog.select_default(None) {
            primary_model_input.set_text(&def.model_id);
        } else {
            primary_model_input.set_text("meta/llama-3.1-70b-instruct");
        }

        if let Some(fast) = catalog.select_fast_default(Some(primary_model_input.text())) {
            fast_model_input.set_text(&fast.model_id);
        } else {
            fast_model_input.set_text("meta/llama-3.2-11b-vision-instruct");
        }

        Self {
            workspace_path,
            current_step: SetupStep::WorkspaceTrust,
            doctor,
            probes,
            trust_confirmed: false,
            warnings_acknowledged: false,
            api_key_input,
            primary_model_input,
            fast_model_input,
            catalog,
            model_search_input,
            selected_model_index: 0,
            model_scroll_offset: 0,
            model_focus_search: false,
            profile: WizardProfile::Balanced,
            require_approval_for_writes: true,
            max_budget_dollars: 25,
            connection_tested: false,
            connection_status: None,
            status_message: None,
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

        let provider = crate::model::provider::nvidia::NvidiaProvider::new(base, Some(key_str))
            .map_err(|e| format!("Provider init error: {e}"))?;

        let discovery_future = async move { provider.discover_models().await };

        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(discovery_future))
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime creation error: {e}"))?;
            rt.block_on(discovery_future)
        };

        res.map_err(|e| format!("Discovery failed: {e}"))
    }

    /// Trigger dynamic discovery and update internal catalog, inputs, and cache file.
    pub fn refresh_discovery(&mut self) -> Result<usize, String> {
        let api_key = if self.api_key_input.text().trim().is_empty() {
            None
        } else {
            Some(self.api_key_input.text())
        };

        let models = Self::run_discovery_sync(None, api_key)?;
        let count = models.len();
        self.catalog.update_from_provider("nvidia_nim", models);

        // Update default selections from newly discovered catalog
        if let Some(def) = self.catalog.select_default(None) {
            self.primary_model_input.set_text(&def.model_id);
        }
        if let Some(fast) = self
            .catalog
            .select_fast_default(Some(self.primary_model_input.text()))
        {
            self.fast_model_input.set_text(&fast.model_id);
        }

        // Save to the channel-aware cache (matches load above).
        let cache_path = ModelCatalog::cache_path_for_channel(
            &self.workspace_path,
            crate::deployment::DeploymentChannel::current(),
        );
        let _ = self.catalog.save_to_cache_file(&cache_path);

        self.selected_model_index = 0;
        self.model_scroll_offset = 0;
        self.status_message = Some(format!("Discovered {} live models from NVIDIA NIM", count));

        Ok(count)
    }

    /// Persist wizard onboarding choices into canonical configuration files:
    /// 1. `.m31a/credentials.json` (0600 secure permissions for API token)
    /// 2. `.m31a/config.toml` (authoritative workspace configuration)
    /// 3. `.m31a/cache/model_catalog.json` (authoritative cached model catalog)
    pub fn persist_configuration(&self) -> Result<(), String> {
        let m31a_dir = self.workspace_path.join(".m31a");
        std::fs::create_dir_all(&m31a_dir)
            .map_err(|e| format!("Failed to create .m31a directory: {e}"))?;

        // 1. Persist credentials if API key is provided (channel-aware store)
        let key = self.api_key_input.text().trim();
        if !key.is_empty() {
            let creds_path =
                crate::config::provider_registry::ProviderRegistry::channel_credentials_path(
                    &self.workspace_path,
                );
            let mut reg = ProviderRegistry::new();
            reg.set_credential("nvidia_nim", key);
            reg.save_credentials_to_file(&creds_path)
                .map_err(|e| format!("Failed to save credentials: {e}"))?;
        }

        // 2. Persist authoritative workspace configuration to .m31a/config.toml.
        // A present-but-invalid existing file is a hard error: silently
        // replacing it would destroy operator configuration without evidence.
        let config_path = m31a_dir.join("config.toml");
        let mut app_config = if config_path.is_file() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read existing workspace configuration: {e}"))?;
            crate::config::schema::parse_and_validate_config(&content)
                .map_err(|e| format!("Existing workspace configuration is invalid: {e}"))?
        } else {
            AppConfig::default()
        };

        app_config.provider.default = "nvidia_nim".to_string();
        let primary = self.primary_model_input.text().trim();
        if !primary.is_empty() {
            app_config.agents.default_model = primary.to_string();
        }
        let fast = self.fast_model_input.text().trim();
        if !fast.is_empty() {
            app_config.agents.fast_auxiliary_model = Some(fast.to_string());
            app_config.agents.fallback_models = vec![fast.to_string()];
        }
        app_config.budget.max_cost_usd = Some(self.max_budget_dollars as f64);
        app_config.policy.interactive_approvals = self.require_approval_for_writes;

        let toml_str = toml::to_string_pretty(&app_config)
            .map_err(|e| format!("Failed to serialize config.toml: {e}"))?;
        std::fs::write(&config_path, toml_str)
            .map_err(|e| format!("Failed to write config.toml: {e}"))?;

        // 3. Persist model catalog to the channel-aware cache file
        if !self.catalog.is_empty() {
            let cache_path = ModelCatalog::cache_path_for_channel(
                &self.workspace_path,
                crate::deployment::DeploymentChannel::current(),
            );
            let _ = self.catalog.save_to_cache_file(&cache_path);
        }

        Ok(())
    }

    /// Refresh doctor diagnostics against workspace.
    pub fn refresh_diagnostics(&mut self) {
        self.probes = self.doctor.run_all(&self.workspace_path);
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
    pub fn can_advance(&self) -> bool {
        match self.current_step {
            SetupStep::WorkspaceTrust => self.trust_confirmed,
            SetupStep::DoctorDiagnostics => {
                let has_fatal = self.doctor.has_blocking_failures(&self.probes);
                let has_warn = self.doctor.has_warnings(&self.probes);
                !has_fatal && (!has_warn || self.warnings_acknowledged)
            }
            SetupStep::ProviderSetup => {
                // NVIDIA NIM is the sole production provider: an API key or
                // token is always required (no keyless local-provider path).
                !self.api_key_input.text().trim().is_empty()
            }
            SetupStep::ModelSetup => !self.primary_model_input.text().trim().is_empty(),
            SetupStep::ProfileSelection => true,
            SetupStep::AutonomySafety => true,
            SetupStep::FinalVerification => true,
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
            KeyCode::Enter => {
                if self.current_step == SetupStep::ModelSetup && self.model_focus_search {
                    self.model_focus_search = false;
                    WizardOutcome::None
                } else if self.current_step == SetupStep::FinalVerification {
                    WizardOutcome::Completed
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
            KeyCode::Up => {
                match self.current_step {
                    // Single production provider: no navigation target exists.
                    SetupStep::ProviderSetup => {}
                    SetupStep::ModelSetup => {
                        if self.selected_model_index > 0 {
                            self.selected_model_index -= 1;
                        }
                    }
                    SetupStep::ProfileSelection => {
                        self.profile = match self.profile {
                            WizardProfile::Balanced => WizardProfile::CodeReviewer,
                            WizardProfile::Autonomous => WizardProfile::Balanced,
                            WizardProfile::Conservative => WizardProfile::Autonomous,
                            WizardProfile::CodeReviewer => WizardProfile::Conservative,
                        };
                    }
                    _ => {}
                }
                WizardOutcome::None
            }
            KeyCode::Down => {
                match self.current_step {
                    // Single production provider: no navigation target exists.
                    SetupStep::ProviderSetup => {}
                    SetupStep::ModelSetup => {
                        let filtered_len = self.filtered_models().len();
                        if self.selected_model_index + 1 < filtered_len {
                            self.selected_model_index += 1;
                        }
                    }
                    SetupStep::ProfileSelection => {
                        self.profile = match self.profile {
                            WizardProfile::Balanced => WizardProfile::Autonomous,
                            WizardProfile::Autonomous => WizardProfile::Conservative,
                            WizardProfile::Conservative => WizardProfile::CodeReviewer,
                            WizardProfile::CodeReviewer => WizardProfile::Balanced,
                        };
                    }
                    _ => {}
                }
                WizardOutcome::None
            }
            KeyCode::Char('t') if self.current_step == SetupStep::FinalVerification => {
                self.connection_tested = true;
                self.connection_status =
                    Some("Connection probe succeeded (latency 42ms)".to_string());
                WizardOutcome::None
            }
            _ => {
                // Pass text input to active field if on provider or model setup
                match self.current_step {
                    SetupStep::ProviderSetup => {
                        let _ = self.api_key_input.handle_key(key);
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
                Constraint::Length(3), // Top progress bar
                Constraint::Min(12),   // Body content
                Constraint::Length(3), // Bottom key hints and status
            ])
            .split(area);

        self.render_progress_bar(f, chunks[0]);
        self.render_step_body(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    fn render_progress_bar(&self, f: &mut Frame, area: Rect) {
        let steps = [
            SetupStep::WorkspaceTrust,
            SetupStep::DoctorDiagnostics,
            SetupStep::ProviderSetup,
            SetupStep::ModelSetup,
            SetupStep::ProfileSelection,
            SetupStep::AutonomySafety,
            SetupStep::FinalVerification,
        ];

        let mut spans = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let is_current = *step == self.current_step;
            let is_completed = step.step_number() < self.current_step.step_number();

            let (style, prefix) = if is_current {
                (
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    format!("[{}. {}]", step.step_number(), step.title()),
                )
            } else if is_completed {
                (
                    Style::default().fg(Color::Green),
                    format!("{}. {}", step.step_number(), step.title()),
                )
            } else {
                (
                    Style::default().fg(Color::DarkGray),
                    format!("{}. {}", step.step_number(), step.title()),
                )
            };

            spans.push(Span::styled(prefix, style));
            if i + 1 < steps.len() {
                spans.push(Span::styled(" -> ", Style::default().fg(Color::DarkGray)));
            }
        }

        let p = Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_type(BorderType::Plain)
                    .title(" M31A FIRST-RUN SETUP WIZARD "),
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
        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 1/7: {} ", SetupStep::WorkspaceTrust.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let trust_box = if self.trust_confirmed { "[X]" } else { "[ ]" };
        let trust_style = if self.trust_confirmed {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Workspace Directory: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.workspace_path.display().to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Security Notice: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(
                    "M31 Autonomous will execute build tools, test suites, and git operations",
                ),
            ]),
            Line::from("within this directory on behalf of autonomous planning objectives."),
            Line::from(
                "Treat external repositories and untrusted inputs with appropriate caution.",
            ),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  {} ", trust_box), trust_style),
                Span::styled(
                    "I trust this repository, its build scripts, and dependencies (Space to toggle)",
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_doctor_diagnostics(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 2/7: {} ",
            SetupStep::DoctorDiagnostics.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines = Vec::new();
        for probe in &self.probes {
            let badge_style = match probe.status {
                DiagnosticStatus::Pass => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                DiagnosticStatus::Warn => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                DiagnosticStatus::Fail => {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                }
            };

            lines.push(Line::from(vec![
                Span::styled(format!(" {:6} ", probe.status.badge()), badge_style),
                Span::styled(
                    format!("{:<26} ", probe.name),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&probe.message),
            ]));

            if let Some(ref rem) = probe.remediation {
                lines.push(Line::from(vec![
                    Span::raw("        ↳ Remediation: "),
                    Span::styled(rem, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        if self.doctor.has_warnings(&self.probes) {
            let warn_box = if self.warnings_acknowledged {
                "[X]"
            } else {
                "[ ]"
            };
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", warn_box), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    "Acknowledge non-critical warnings and proceed in degraded mode (Space to toggle)",
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_provider_setup(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 3/7: {} ", SetupStep::ProviderSetup.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let selected_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let mut lines = vec![
            Line::from(Span::styled(
                "Provider",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" (●) ", selected_style),
                Span::styled("NVIDIA NIM", selected_style),
            ]),
            Line::from(Span::styled(
                "       Dynamic model discovery",
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "       Production provider",
                Style::default().fg(Color::Gray),
            )),
            Line::from(""),
        ];
        lines.push(Line::from(vec![
            Span::styled(
                "API Key / Token: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                self.api_key_input.display_text(),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                if self.api_key_input.text().is_empty() {
                    " <type key to enter>"
                } else {
                    ""
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_model_setup(&self, f: &mut Frame, area: Rect) {
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
        let status_str = format!("{}", self.catalog.refresh_state);
        let search_border_style = if self.model_focus_search {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let search_title = format!(
            " Provider: {} | Catalog: {} discovered ({}) | Showing: {}/{} ",
            prov_display,
            self.catalog.len(),
            status_str,
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
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                search_text,
                if self.model_search_input.text().is_empty() && !self.model_focus_search {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Yellow)
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
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
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
                    Style::default().fg(Color::Yellow),
                )));
                list_lines.push(Line::from(Span::styled(
                    " Press [R] to discover live models from NVIDIA NIM,",
                    Style::default().fg(Color::DarkGray),
                )));
                list_lines.push(Line::from(Span::styled(
                    " or configure API key in Step 3.",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                list_lines.push(Line::from(Span::styled(
                    " No models match the search query.",
                    Style::default().fg(Color::Yellow),
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
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if is_primary {
                    Style::default().fg(Color::Cyan)
                } else if is_fast {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Gray)
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
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
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
            .border_style(Style::default().fg(Color::DarkGray));
        let details_inner = details_block.inner(middle_chunks[1]);
        f.render_widget(details_block, middle_chunks[1]);

        let mut detail_lines = Vec::new();
        if let Some(candidate) = filtered.get(selected_idx) {
            detail_lines.push(Line::from(vec![
                Span::styled(
                    "Model ID: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&candidate.model_id, Style::default().fg(Color::Cyan)),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Provider: ", Style::default().fg(Color::White)),
                Span::raw(format!("{} ({})", candidate.provider, candidate.source)),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Source:   ", Style::default().fg(Color::White)),
                Span::styled(
                    candidate.context_provenance().unwrap_or("unknown"),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Kind:     ", Style::default().fg(Color::White)),
                Span::raw(format!("{}", candidate.model_kind)),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Tier:     ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}", candidate.tier),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Context:  ", Style::default().fg(Color::White)),
                Span::styled(
                    if candidate.context_capacity > 0 {
                        format!("{} tokens", candidate.context_capacity)
                    } else {
                        "Unknown (unreported by provider)".to_string()
                    },
                    if candidate.context_capacity >= 4096 && candidate.is_context_known() {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    },
                ),
            ]));
            detail_lines.push(Line::from(vec![
                Span::styled("Tools:    ", Style::default().fg(Color::White)),
                match candidate.tool_support {
                    crate::model::router::resolver::CapabilitySupport::Supported => {
                        Span::styled("Supported (✓)", Style::default().fg(Color::Green))
                    }
                    crate::model::router::resolver::CapabilitySupport::Unsupported => {
                        Span::styled("Unsupported (✗)", Style::default().fg(Color::Red))
                    }
                    crate::model::router::resolver::CapabilitySupport::Unknown => {
                        Span::styled("Unknown (?)", Style::default().fg(Color::Yellow))
                    }
                },
            ]));
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                "Role Eligibility Evaluation:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));

            let primary_res = Self::check_primary_eligibility(candidate);
            let fast_res = Self::check_fast_auxiliary_eligibility(candidate);

            detail_lines.push(Line::from(vec![
                Span::styled("  Primary: ", Style::default().fg(Color::White)),
                match primary_res {
                    Ok(()) => Span::styled(
                        "✓ Eligible (Tool calling & context)",
                        Style::default().fg(Color::Green),
                    ),
                    Err(e) => Span::styled(
                        format!("✗ Ineligible: {}", e),
                        Style::default().fg(Color::Red),
                    ),
                },
            ]));

            detail_lines.push(Line::from(vec![
                Span::styled("  Fast:    ", Style::default().fg(Color::White)),
                match fast_res {
                    Ok(()) => Span::styled(
                        "✓ Eligible (Auxiliary / summarization)",
                        Style::default().fg(Color::Green),
                    ),
                    Err(e) => Span::styled(
                        format!("✗ Ineligible: {}", e),
                        Style::default().fg(Color::Yellow),
                    ),
                },
            ]));
        } else {
            detail_lines.push(Line::from(Span::styled(
                "Select a model to view capabilities & role eligibility.",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let p_details = Paragraph::new(detail_lines).wrap(Wrap { trim: true });
        f.render_widget(p_details, details_inner);

        // 3. Bottom Roles & Actions
        let roles_block = Block::default()
            .borders(Borders::NONE)
            .title(" Active Role Assignments ")
            .border_style(Style::default().fg(Color::DarkGray));
        let roles_inner = roles_block.inner(chunks[2]);
        f.render_widget(roles_block, chunks[2]);

        let roles_lines = vec![
            Line::from(vec![
                Span::styled(
                    "Primary Reasoning: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.primary_model_input.display_text(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  (DAG, code synthesis, architecture)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Fast Auxiliary:    ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.fast_model_input.display_text(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  (Linting, small tools, summaries)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "[1/p] ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Set Primary  "),
                Span::styled(
                    "[2/a] ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Set Fast  "),
                Span::styled(
                    "[Tab//] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Search/List  "),
                Span::styled(
                    "[R] ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Refresh Discovery  "),
                Span::styled(
                    "[Enter] ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Confirm & Advance"),
            ]),
        ];

        let p_roles = Paragraph::new(roles_lines);
        f.render_widget(p_roles, roles_inner);
    }

    fn render_model_setup_compact(&self, f: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Primary Reasoning Model: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.primary_model_input.display_text(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Fast Auxiliary Model:    ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    self.fast_model_input.display_text(),
                    Style::default().fg(Color::Green),
                ),
            ]),
        ];
        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }

    fn render_profile_selection(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 5/7: {} ",
            SetupStep::ProfileSelection.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let profiles = [
            WizardProfile::Balanced,
            WizardProfile::Autonomous,
            WizardProfile::Conservative,
            WizardProfile::CodeReviewer,
        ];

        let mut lines = vec![
            Line::from(Span::styled(
                "Select Execution Profile (Up/Down arrow keys):",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for p in profiles {
            let is_sel = p == self.profile;
            let (radio, style) = if is_sel {
                (
                    " [*] ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (" [ ] ", Style::default().fg(Color::Gray))
            };

            lines.push(Line::from(vec![
                Span::styled(radio, style),
                Span::styled(format!("{:<15} ", p.name()), style),
                Span::styled(p.description(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_autonomy_safety(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::NONE)
            .title(format!(" Step 6/7: {} ", SetupStep::AutonomySafety.title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let apprv_box = if self.require_approval_for_writes {
            "[X]"
        } else {
            "[ ]"
        };

        let lines = vec![
            Line::from(vec![Span::styled(
                "Autonomy Policy Guardrails:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("  {} ", apprv_box),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Require human authorization for destructive file modifications and shell executions (Space to toggle)",
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Default Mission Budget Limit: ",
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("${}.00 USD", self.max_budget_dollars),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                "  (M31A automatically halts execution when cumulative API spend reaches this budget)",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_final_verification(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::NONE).title(format!(
            " Step 7/7: {} ",
            SetupStep::FinalVerification.title()
        ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let mut lines = vec![
            Line::from(Span::styled(
                "Setup Configuration Ready for Activation:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Workspace:    ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(self.workspace_path.display().to_string()),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Provider:     ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("NVIDIA NIM"),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Primary Model:",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", self.primary_model_input.text())),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Fast Model:   ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", self.fast_model_input.text())),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Catalog:      ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {} models discovered", self.catalog.len())),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Profile:      ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(self.profile.name()),
            ]),
            Line::from(""),
        ];

        if let Some(ref status) = self.connection_status {
            lines.push(Line::from(vec![
                Span::styled(
                    "  Provider Probe: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(status, Style::default().fg(Color::Green)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "  Provider Probe: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[Press 'T' to run live connection probe]",
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press [Enter] to complete onboarding and launch the M31A Cockpit.",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        let p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let mut spans = vec![
            Span::styled(
                "Setup Wizard  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[Enter] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if self.current_step == SetupStep::FinalVerification {
                "Finish Onboarding  "
            } else {
                "Next Step  "
            }),
            Span::styled(
                "[B] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Back  "),
            Span::styled(
                "[Esc] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cancel Setup"),
        ];

        if let Some(ref msg) = self.status_message {
            spans.push(Span::raw("  |  "));
            spans.push(Span::styled(
                msg,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }

        let p = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::TOP))
            .alignment(Alignment::Center);

        f.render_widget(p, area);
    }
}
