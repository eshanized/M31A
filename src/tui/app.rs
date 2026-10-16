//! The one TUI application root (L9 projection owner).
//!
//! `TuiApplication` owns every presentation responsibility: UI state,
//! runtime binding, routing, input, transient UI, and rendering.
//! Enforces Law 9: "The TUI is a projection, never authoritative state."
//! Features dirty-state frame scheduling (30–60 FPS), sub-5ms render budget,
//! bounded update consumption, and zero database queries during draw.
//!
//! Construction is synchronous and runtime-free: `new()` builds a valid
//! engine, `with_runtime_startup()` arms the production startup state, the
//! first frame draws before any runtime await, and the canonical `AppRuntime`
//! (assembled by the real composition root in `main.rs`) attaches later via
//! the binding. Events flow `runtime → bridge → TuiEvent → apply_tui_event
//! → render`; `render` performs zero I/O and never touches the runtime.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

use super::approval::ApprovalModal;
use super::binding::{RuntimeAssemblyOutcome, TuiRuntimeBinding};
use super::channel::{ActionSendError, TuiActionSender, TuiInteractionReceiver};
use super::composer::{ComposerAction, TuiComposer};
use super::conversation::TuiConversationItem;
use super::errors::TuiError;
use super::focus::{FocusManager, FocusTarget};
use super::keymap::{KeyAction, KeyContext, resolve_key};
use super::layout::compute_layout;
use super::lifecycle::TuiLifecycleStage;
use super::model::{ActivityKind, TuiViewModel};
use super::navigation::{NavigationAction, NavigationRouter, ScreenId};
use super::overlay::{OverlayManager, render_help_overlay};
use super::palette_v2::{PaletteActionV2, UniversalCommandPalette};
use super::registry::ViewId;
use super::replay::ReplayController;
use super::screens::wizard::{SetupWizardScreen, WizardOutcome};
use super::shell::{render_header, render_workspace};
use super::surface::{
    ModelSelectorAction, ModelSelectorState, WorkflowAction, WorkflowDashboardState,
    handle_model_selector_key, handle_workflow_dashboard_key,
};
use super::theme::{ThemeMode, ThemeTokens};
use crate::cli::RuntimeCommand;
use crate::events::envelope::EventEnvelope;
use crate::interaction::action::ApplicationAction;
use crate::interaction::events::InteractionEvent;

/// TUI application root: the single owner of presentation state,
/// runtime binding, routing, input, transient UI, and rendering.
pub struct TuiApplication {
    pub model: TuiViewModel,
    pub navigation: NavigationRouter,
    pub palette: UniversalCommandPalette,
    pub approval_modal: ApprovalModal,
    pub overlay_manager: OverlayManager,
    pub replay: ReplayController,
    pub replay_model: Option<TuiViewModel>,
    pub focus: FocusManager,
    pub context_selected_idx: usize,
    pub diff_scroll_offset: usize,
    pub is_running: bool,
    pub force_redraw: bool,
    pub composer: TuiComposer,
    pub is_composer_focused: bool,
    pub interaction_rx: Option<TuiInteractionReceiver>,
    pub bridge_tx: Option<TuiActionSender>,
    pub has_event_backlog: bool,
    pub theme_mode: ThemeMode,
    /// Workflow dashboard interaction state
    pub workflow_dashboard_state: WorkflowDashboardState,
    /// Cached workflow snapshot for dashboard rendering
    pub workflow_snapshot: Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
    /// Setup Wizard state - accessible via View 22
    pub setup_wizard: Option<SetupWizardScreen>,
    /// Model selector state - accessible via View 24
    pub model_selector_state: ModelSelectorState,
    /// Target FPS for frame rendering and loop polling
    pub target_fps: u32,
    /// Settings screen editor state
    pub settings_state: crate::tui::surface::settings::SettingsState,
    /// Canonical resolved configuration backing settings and runtime overrides
    pub resolved_config: Option<std::sync::Arc<crate::config::ResolvedConfiguration>>,
    /// Mutable draft configuration for interactive editing before save
    pub settings_draft: Option<crate::config::schema::AppConfig>,
    /// Canonical runtime binding: attached runtime + bridge channels.
    /// `None` until the composition root's runtime attaches.
    pub binding: TuiRuntimeBinding,
    /// Async runtime-assembly receiver from the real composition root.
    assembly_rx: Option<UnboundedReceiver<RuntimeAssemblyOutcome>>,
    assembly_settled: bool,
    pub assembly_started_at: Option<std::time::Instant>,
    pub assembly_timeout: std::time::Duration,
    pub assembly_abort_handle: Option<tokio::task::AbortHandle>,
    /// Bounded live startup-progress channel (latest stage only, never a
    /// growing backlog). `None` for legacy `begin_assembly` callers.
    assembly_progress_rx:
        Option<tokio::sync::watch::Receiver<crate::startup_progress::StartupStageUpdate>>,
    /// Last observed startup stage (for timeout diagnostics and live display).
    /// `None` when no progress update has arrived yet — distinguished from
    /// an actively-running stage in timeout messages.
    assembly_last_stage: Option<crate::startup_progress::StartupStageUpdate>,
    /// Assembly generation: incremented on every `begin_assembly*` call.
    /// A delayed completion from a timed-out/aborted attempt can never
    /// attach after a retry because the channel is replaced and the
    /// generation is checked before attaching.
    assembly_generation: u64,
    /// Generation of the in-flight attempt (matches `assembly_generation`
    /// while an attempt is active).
    assembly_active_generation: u64,
    /// Last observed terminal viewport area.
    pub last_area: Rect,
}

impl Default for TuiApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApplication {
    pub fn new() -> Self {
        Self {
            model: TuiViewModel::new(),
            navigation: NavigationRouter::new(),
            palette: UniversalCommandPalette::new(),
            approval_modal: ApprovalModal::new(),
            overlay_manager: OverlayManager::new(),
            replay: ReplayController::new(),
            replay_model: None,
            focus: FocusManager::new(FocusTarget::Composer),
            context_selected_idx: 0,
            diff_scroll_offset: 0,
            is_running: true,
            force_redraw: true,
            composer: TuiComposer::new(PathBuf::from(".")),
            is_composer_focused: true,
            interaction_rx: None,
            bridge_tx: None,
            has_event_backlog: false,
            // Pre-hydration placeholder: always overwritten by `with_config`
            // (resolved configuration) before production render. Uses the
            // canonical default so even an unhydrated app agrees with the
            // resolver on the effective theme.
            theme_mode: ThemeMode::canonical_default(),
            workflow_dashboard_state: WorkflowDashboardState::new(),
            workflow_snapshot: None,
            setup_wizard: None,
            model_selector_state: ModelSelectorState::new(),
            target_fps: crate::config::canonical::DEFAULT_TUI_FPS,
            settings_state: crate::tui::surface::settings::SettingsState::new(),
            resolved_config: None,
            settings_draft: None,
            binding: TuiRuntimeBinding::new(),
            assembly_rx: None,
            assembly_settled: false,
            assembly_started_at: None,
            assembly_timeout: std::time::Duration::from_secs(
                crate::config::canonical::DEFAULT_RUNTIME_ASSEMBLY_TIMEOUT_SECS,
            ),
            assembly_abort_handle: None,
            assembly_progress_rx: None,
            assembly_last_stage: None,
            assembly_generation: 0,
            assembly_active_generation: 0,
            last_area: Rect::new(0, 0, 120, 40),
        }
    }

    /// Production startup constructor: synchronous, runtime-free.
    ///
    /// Arms the `InitializingRuntime` presentation state; the caller must
    /// draw the first frame immediately (see `render_first_frame`) before
    /// any runtime await.
    pub fn with_runtime_startup(
        self,
        workspace_root: PathBuf,
        config: &crate::config::ResolvedConfiguration,
    ) -> Self {
        let mut this = self
            .with_workspace_root(workspace_root)
            .with_config(config)
            .with_composer_focused(true);
        this.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
        this
    }

    pub fn with_interaction_rx(mut self, rx: impl Into<TuiInteractionReceiver>) -> Self {
        self.interaction_rx = Some(rx.into());
        self
    }

    pub fn with_bridge_tx(mut self, tx: impl Into<TuiActionSender>) -> Self {
        self.bridge_tx = Some(tx.into());
        self
    }

    /// Send an action to the governed runtime bridge, failing EXPLICITLY.
    ///
    /// A dropped/failed channel send must never become a silent Thinking
    /// state: the request is revoked into a visible error instead.
    /// Returns true when the runtime accepted the action.
    fn send_or_fail(&mut self, op: &str, action: ApplicationAction) -> bool {
        let outcome = self.bridge_tx.as_ref().map(|tx| tx.try_send(action));
        match outcome {
            Some(Ok(())) => true,
            Some(Err(ActionSendError::Full)) => {
                self.model.fail_request("Bridge queue full (backpressure)");
                self.model.add_conversation_item(
                    crate::tui::conversation::TuiConversationItem::Error {
                        message: format!(
                            "Cannot run {op}: governed runtime bridge queue is full; action rejected under backpressure."
                        ),
                        timestamp: chrono::Utc::now(),
                    },
                );
                self.model.add_log(
                    "ERROR",
                    "ApplicationAction dropped: bridge channel queue full (fail-closed)",
                    "tui",
                );
                false
            }
            Some(Err(ActionSendError::Closed)) | None => {
                self.model.fail_request("Bridge send failed");
                self.model.add_conversation_item(
                    crate::tui::conversation::TuiConversationItem::Error {
                        message: format!(
                            "Cannot run {op}: governed runtime bridge unavailable; execution blocked."
                        ),
                        timestamp: chrono::Utc::now(),
                    },
                );
                self.model.add_log(
                    "ERROR",
                    "ApplicationAction dropped: bridge channel send failed (fail-closed)",
                    "tui",
                );
                false
            }
        }
    }

    /// Returns true if the context rail / split panel is visible at current screen and terminal size.
    pub fn has_context_panel(&self) -> bool {
        // Focus eligibility must match the responsive renderer: below the
        // split-layout breakpoint there is no visible context panel to focus.
        if self.last_area.width < 120 {
            return false;
        }
        match self.navigation.current_screen {
            ScreenId::Dashboard => !self.model.session_view_mode.is_welcome(),
            ScreenId::TaskGraph
            | ScreenId::Agents
            | ScreenId::Tools
            | ScreenId::Git
            | ScreenId::Verification
            | ScreenId::Jobs
            | ScreenId::Doctor
            | ScreenId::ModelUsage
            | ScreenId::Artifacts => true,
            _ => false,
        }
    }

    /// Normalize focus when terminal resizing or screen changes hide the focused panel.
    pub fn normalize_focus(&mut self) {
        if self.navigation.current_screen == ScreenId::Settings {
            if self.focus.current() != FocusTarget::Settings
                && self.focus.current() != FocusTarget::Overlay
            {
                self.focus.enter_settings();
                self.is_composer_focused = false;
            }
        } else if self.focus.is_settings() {
            self.focus.exit_settings();
        }
        if self.focus.current() == FocusTarget::ContextPanel && !self.has_context_panel() {
            self.focus.set_focus(FocusTarget::Conversation);
            self.is_composer_focused = false;
            self.model.mark_dirty();
        }
    }

    /// Clamp and normalize stored context rail selection and scroll offsets.
    pub fn normalize_context_rail_state(&mut self) {
        let total_lines = crate::tui::shell::context_rail::calculate_total_rail_lines(&self.model);
        if total_lines == 0 {
            self.context_selected_idx = 0;
            self.diff_scroll_offset = 0;
        } else {
            self.context_selected_idx =
                self.context_selected_idx.min(total_lines.saturating_sub(1));
            let viewport_h = self.last_area.height.saturating_sub(5) as usize;
            let max_scroll = total_lines.saturating_sub(viewport_h.max(1));
            self.diff_scroll_offset = self.diff_scroll_offset.min(max_scroll);
        }
    }

    /// Set simulated or actual terminal size (used for testing and resize events).
    pub fn set_terminal_size(&mut self, width: u16, height: u16) {
        self.last_area = Rect::new(0, 0, width, height);
        self.force_redraw = true;
        self.normalize_focus();
        self.normalize_context_rail_state();
    }

    /// Authoritative resize handler dispatched from event loop.
    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.set_terminal_size(width, height);
        self.model.mark_dirty();
    }

    /// Scroll the authoritative conversation viewport from a mouse wheel.
    /// Wheel-up moves toward older content; wheel-down toward newer.
    /// No-op while any modal, overlay, or wizard owns input.
    pub fn handle_mouse_scroll(&mut self, up: bool) {
        // Preserve the legacy keyboard/test entry point by routing through the
        // center of the main viewport.
        let x = self.last_area.x.saturating_add(self.last_area.width / 2);
        let y = self.last_area.y.saturating_add(self.last_area.height / 2);
        self.handle_mouse_scroll_at(up, x, y);
    }

    /// Route wheel input to the visible pane under the pointer.
    pub fn handle_mouse_scroll_at(&mut self, up: bool, x: u16, y: u16) {
        if self.palette.is_open
            || self.approval_modal.is_open
            || self.overlay_manager.is_help_open
            || self.navigation.active_modal.is_some()
            || self.navigation.active_overlay.is_some()
            || self.is_wizard_active()
        {
            return;
        }

        let (_tier, areas) = crate::tui::layout::compute_layout(self.last_area);
        let contains = |area: ratatui::layout::Rect| {
            x >= area.x
                && x < area.x.saturating_add(area.width)
                && y >= area.y
                && y < area.y.saturating_add(area.height)
        };

        if self.navigation.active_detail.is_some() && contains(areas.main) {
            if up {
                self.model.detail_scroll_offset = self.model.detail_scroll_offset.saturating_sub(1);
            } else {
                self.model.detail_scroll_offset = self.model.detail_scroll_offset.saturating_add(1);
            }
            self.model.mark_dirty();
            return;
        }

        if areas.sidebar.is_some_and(contains) && self.has_context_panel() {
            if up {
                self.context_selected_idx = self.context_selected_idx.saturating_sub(1);
                self.diff_scroll_offset = self.diff_scroll_offset.saturating_sub(1);
            } else {
                self.context_selected_idx = self.context_selected_idx.saturating_add(1);
                self.diff_scroll_offset = self.diff_scroll_offset.saturating_add(1);
            }
            self.normalize_context_rail_state();
            self.model.mark_dirty();
            return;
        }

        if contains(areas.main) {
            self.model.scroll_wheel(up);
        }
    }

    pub fn with_workspace_root(mut self, root: PathBuf) -> Self {
        self.model.workspace_path = root.display().to_string();
        self.composer.set_workspace_root(root);
        self
    }

    /// Configure TUI settings (FPS, theme, model) from authoritative ResolvedConfiguration.
    pub fn with_config(mut self, config: &crate::config::ResolvedConfiguration) -> Self {
        self.target_fps = config.app_config.tui.fps.clamp(1, 120);
        let mode = ThemeMode::from_str_relaxed(&config.app_config.tui.theme);
        self.theme_mode = mode;
        self.model.active_model = config.active_model.clone();
        self.model.active_provider = config.active_provider.clone();
        if let Some(ref prof) = config.active_profile {
            self.model.active_profile = prof.clone();
        }
        self.resolved_config = Some(std::sync::Arc::new(config.clone()));
        self.settings_draft = Some(config.app_config.clone());
        self.model.fast_auxiliary_model = config.app_config.agents.fast_auxiliary_model.clone();
        // Capture only presence bits for supported non-secret overrides at the
        // configuration boundary. Renderers and input handlers never inspect env.
        let override_keys = [
            "M31A_MODEL",
            "M31A_CONCURRENCY",
            "M31A_TIMEOUT_SECS",
            "M31A_THEME",
            "M31A_MAX_STEPS",
        ];
        self.model.environment_overrides = override_keys
            .iter()
            .map(|key| (key.to_string(), std::env::var_os(key).is_some()))
            .collect();
        // Credential presence is resolved during runtime hydration, not while
        // constructing the first-frame UI.
        self.model.credential_status = crate::tui::model::TuiCredentialStatus::Loading;
        self
    }

    /// Target frame interval derived from configured FPS (bounded 1..=120).
    pub fn frame_interval(&self) -> std::time::Duration {
        let fps = self.target_fps.clamp(1, 120);
        std::time::Duration::from_micros((1_000_000 / fps as u64).max(1))
    }

    /// Authoritative poll interval for the TUI event loop.
    ///
    /// When active animations exist, polls at the configured frame interval
    /// so animations render smoothly at the target frame rate.
    /// When idle, respects the configured frame rate without busy-looping:
    /// never polls faster than the configured frame interval, relaxed to
    /// at least 50ms (or configured interval if configured slower than 20 FPS).
    pub fn poll_interval(&self) -> std::time::Duration {
        if self.has_event_backlog {
            return std::time::Duration::ZERO;
        }
        let frame = self.frame_interval();
        if self.model.has_active_animation() {
            frame
        } else {
            frame.max(std::time::Duration::from_millis(50))
        }
    }

    /// Asynchronously hydrate TUI model with durable runtime truth (zero render-time I/O, P0-P2).
    pub async fn hydrate_from_runtime(&mut self, runtime: &crate::runtime::AppRuntime) {
        // 1. Artifacts from canonical ArtifactService
        match runtime.artifact_service().list_all().await {
            Ok(records) => {
                let snaps = records
                    .iter()
                    .map(crate::tui::model::TuiArtifactSnapshot::from)
                    .collect();
                self.model.load_artifacts(snaps);
            }
            Err(e) => {
                self.model.set_quarantine_failed(e.to_string());
            }
        }

        // 1b. Real sandbox status probed truthfully from runtime
        self.model
            .set_sandbox_status(runtime.probe_sandbox_status());

        // 1c. Credential resolution runs on a blocking worker and only projects
        // presence; secret material never enters the TUI model.
        if let Some(cfg) = self.resolved_config.clone() {
            let workspace = cfg.workspace_root.clone();
            let channel = crate::deployment::DeploymentChannel::current();
            match tokio::task::spawn_blocking(move || {
                crate::runtime_authorities::resolve_runtime_credentials(&workspace, channel)
            })
            .await
            {
                Ok(creds) => {
                    self.model.credential_status = if creds.api_key.is_some() {
                        crate::tui::model::TuiCredentialStatus::Configured
                    } else {
                        crate::tui::model::TuiCredentialStatus::NotConfigured
                    };
                }
                Err(error) => {
                    self.model.credential_status =
                        crate::tui::model::TuiCredentialStatus::Unavailable(error.to_string());
                }
            }
        }

        // 2. Verification checks from EvidenceCompletionGate
        match runtime.completion_gate().list_all_checks().await {
            Ok(checks) => {
                let snaps = checks
                    .iter()
                    .map(crate::tui::model::TuiVerificationCheck::from)
                    .collect();
                self.model.load_verification_checks(snaps);
                self.model.verification_section_state =
                    crate::tui::model::SectionProjectionState::Ready;
            }
            Err(e) => {
                self.model.load_verification_checks(Vec::new());
                self.model.verification_section_state =
                    crate::tui::model::SectionProjectionState::Failed(e.to_string());
            }
        }

        // 3. Recovery attempts from RecoveryBudgetTracker
        match crate::recovery::budget::RecoveryBudgetTracker::query_all_attempts(runtime.pool())
            .await
        {
            Ok(attempts) => {
                let snaps = attempts
                    .iter()
                    .map(crate::tui::model::TuiRecoverySnapshot::from)
                    .collect();
                self.model.load_recovery_attempts(snaps);
                self.model.recovery_section_state =
                    crate::tui::model::SectionProjectionState::Ready;
            }
            Err(e) => {
                self.model.load_recovery_attempts(Vec::new());
                self.model.recovery_section_state =
                    crate::tui::model::SectionProjectionState::Failed(e.to_string());
            }
        }

        // 5. Canonical tools from ToolRegistry
        let tools = runtime.tool_registry().list_tools();
        let snaps = tools
            .iter()
            .map(|t| crate::tui::model::TuiToolDefinitionSnapshot::from(t.as_ref()))
            .collect();
        self.model.load_canonical_tools(snaps);

        // 6. Discover skills and workspace files in one bounded blocking worker.
        // This work never runs in a constructor, renderer, or key handler.
        let discovery_root = PathBuf::from(&self.model.workspace_path);
        let skill_root = discovery_root.clone();
        match tokio::task::spawn_blocking(move || {
            let skills =
                crate::skill::discovery::SkillDiscovery::discover_all(Some(skill_root.as_path()));
            let files = TuiComposer::discover_workspace_files(discovery_root);
            (skills, files)
        })
        .await
        {
            Ok((skills, files)) => {
                let snaps = skills
                    .iter()
                    .map(crate::tui::model::TuiSkillSnapshot::from)
                    .collect();
                self.model.load_skills(snaps);
                self.composer.set_discovered_files(files);
            }
            Err(error) => {
                self.model.load_skills(Vec::new());
                self.composer.set_discovered_files(Vec::new());
                self.model.add_log(
                    "WARN",
                    format!("Workspace discovery failed: {error}"),
                    "tui",
                );
            }
        }

        // 7. Authoritative Git state via GitService
        match runtime.git_service().status().await {
            Ok(status) => {
                self.model.git_branch = if status.branch.is_empty() {
                    "N/A".to_string()
                } else {
                    status.branch
                };
                self.model.git_section_state = crate::tui::model::SectionProjectionState::Ready;
            }
            Err(e) => {
                self.model.git_branch = "Error".to_string();
                self.model.git_section_state =
                    crate::tui::model::SectionProjectionState::Failed(e.to_string());
            }
        }

        // 8. Authoritative active task graph if one exists for the active mission
        use crate::persistence::sqlite::repositories::TaskGraphRepository;
        let graph_repo = crate::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(
            runtime.pool().clone(),
        );
        let graph_res = if let Some(ref mid_str) = self.model.mission_id {
            if let Ok(mid) = mid_str.parse::<crate::ids::MissionId>() {
                graph_repo.get_active_graph(mid).await
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        };
        match graph_res {
            Ok(Some(graph)) => {
                let snaps = graph
                    .task_summaries()
                    .iter()
                    .map(|t| crate::tui::model::TuiTaskSnapshot {
                        id: t.id.to_string(),
                        title: t.title.clone(),
                        status: t.status.to_string(),
                        agent_role: Some(t.role.to_string()),
                        progress_pct: if t.status
                            == crate::state_machine::task::TaskState::Succeeded
                        {
                            100
                        } else {
                            0
                        },
                        dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
                        ..Default::default()
                    })
                    .collect();
                self.model.load_tasks(snaps);
                self.model.task_graph_section_state =
                    crate::tui::model::SectionProjectionState::Ready;
            }
            Ok(None) => {
                self.model.load_tasks(Vec::new());
                self.model.task_graph_section_state =
                    crate::tui::model::SectionProjectionState::Ready;
            }
            Err(e) => {
                self.model.load_tasks(Vec::new());
                self.model.task_graph_section_state =
                    crate::tui::model::SectionProjectionState::Failed(e.to_string());
            }
        }

        // 9. Authoritative model usage telemetry (idempotent, prevents double counting)
        let inv_repo = crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
            runtime.pool().clone(),
        );
        match inv_repo.get_all_invocations().await {
            Ok(records) => {
                self.model.hydrate_persisted_usage(&records);
                self.model.usage_section_state = crate::tui::model::SectionProjectionState::Ready;
            }
            Err(e) => {
                self.model.model_usage = crate::tui::model::TuiModelUsage::default();
                self.model.usage_section_state =
                    crate::tui::model::SectionProjectionState::Failed(e.to_string());
            }
        }

        // 10. Load canonical model catalog authority (global cache,
        // legacy workspace cache as migration fallback) — same source the
        // runtime refresh persists to, so CLI/TUI/wizard observe one catalog.
        let catalog_root = PathBuf::from(&self.model.workspace_path);
        match tokio::task::spawn_blocking(move || {
            crate::model::catalog::ModelCatalog::load_canonical_for_workspace(&catalog_root)
        })
        .await
        {
            Ok(Some(catalog)) => {
                let composer_models = catalog
                    .models
                    .iter()
                    .map(|model| {
                        (
                            model.model_id.clone(),
                            format!(
                                "{} | Tier: {} | Ctx: {}",
                                model.provider, model.tier, model.context_capacity
                            ),
                        )
                    })
                    .collect();
                self.model.catalog_models = catalog.models.clone();
                self.model.catalog_snapshot = Some(catalog);
                self.model.catalog_section_state = crate::tui::model::SectionProjectionState::Ready;
                self.composer.refresh_from_catalog(composer_models);
            }
            Ok(None) => {
                self.model.catalog_models.clear();
                self.model.catalog_snapshot = None;
                self.model.catalog_section_state =
                    crate::tui::model::SectionProjectionState::Failed(
                        "Model catalog not found or failed to load".to_string(),
                    );
                self.composer.refresh_from_catalog(Vec::new());
            }
            Err(error) => {
                self.model.catalog_models.clear();
                self.model.catalog_snapshot = None;
                self.model.catalog_section_state =
                    crate::tui::model::SectionProjectionState::Failed(
                        format!("Model catalog discovery failed: {error}"),
                    );
                self.composer.refresh_from_catalog(Vec::new());
            }
        }

        // 11. Authoritative slash command registry into composer and palette
        let snapshot_handle = runtime.command_snapshot_handle();
        self.composer.set_snapshot_handle(snapshot_handle);
        let slash_reg = runtime.slash_registry();
        self.composer.set_slash_registry(slash_reg.clone());
        self.palette.register_slash_commands(slash_reg.as_ref());

        self.model.mark_dirty();
    }

    /// Live reload user commands into TuiComposer and UniversalCommandPalette without recreating TuiApplication.
    pub fn reload_commands(&mut self, runtime: &crate::runtime::AppRuntime) {
        let snapshot_handle = runtime.command_snapshot_handle();
        self.composer.set_snapshot_handle(snapshot_handle);
        let slash_reg = runtime.slash_registry();
        self.composer.set_slash_registry(slash_reg.clone());
        self.palette.register_slash_commands(slash_reg.as_ref());
        self.model.mark_dirty();
    }

    /// Configure initial authoritative slash command registry.
    pub fn with_slash_registry(
        mut self,
        registry: std::sync::Arc<crate::interaction::commands::SlashCommandRegistry>,
    ) -> Self {
        self.palette.register_slash_commands(registry.as_ref());
        self.composer.set_slash_registry(registry);
        self
    }

    /// Focus the interactive prompt composer.
    pub fn focus_composer(&mut self) {
        self.is_composer_focused = true;
        self.focus.set_focus(FocusTarget::Composer);
        self.model.mark_dirty();
    }

    /// Unfocus the interactive prompt composer, returning to navigation mode.
    pub fn unfocus_composer(&mut self) {
        self.is_composer_focused = false;
        self.focus.unfocus_composer();
        self.model.mark_dirty();
    }

    /// Open a contextual detail inspector, updating focus and scroll offset.
    pub fn open_detail(&mut self, detail: ViewId) {
        self.navigation.open_detail(detail);
        self.is_composer_focused = false;
        self.focus.set_focus(FocusTarget::Conversation);
        self.model.detail_scroll_offset = 0;
        self.model.mark_dirty();
    }

    /// Close the active detail inspector and reset scroll offset.
    pub fn close_detail(&mut self) {
        self.navigation.close_detail();
        self.model.detail_scroll_offset = 0;
        self.model.mark_dirty();
    }

    /// Authoritative view navigation for the application.
    /// Opens live floating overlays for HelpDocs and CommandPalette,
    /// sets active detail and updates focus for detail views,
    /// and navigates screens via the navigation controller.
    pub fn navigate_to_view(&mut self, view_id: ViewId) {
        if view_id == ViewId::HelpDocs {
            self.overlay_manager.open_help();
            self.focus.enter_overlay();
        } else if view_id == ViewId::CommandPalette {
            self.palette.open();
            self.focus.enter_overlay();
        } else {
            self.navigation.navigate_to_view(view_id);
            if self.navigation.active_detail.is_some() {
                self.is_composer_focused = false;
                self.focus.set_focus(FocusTarget::Conversation);
                self.model.detail_scroll_offset = 0;
            }
        }
        self.model.mark_dirty();
    }

    /// Programmatically submit text through the composer input pipeline.
    pub fn submit_composer_text(&mut self, text: impl AsRef<str>) -> Option<RuntimeCommand> {
        self.focus_composer();
        self.composer.set_text(text.as_ref());
        self.handle_composer_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    /// Configure initial composer focus.
    pub fn with_composer_focused(mut self, focused: bool) -> Self {
        self.is_composer_focused = focused;
        if focused {
            self.focus.set_focus(FocusTarget::Composer);
        } else {
            self.focus.set_focus(FocusTarget::Conversation);
        }
        self
    }

    /// Mark the TUI as booting before the first frame.
    ///
    /// Keeps `is_running`, `force_redraw`, navigation, theme, and composer
    /// intact: only the explicit startup state changes.
    pub fn set_runtime_booting(&mut self) {
        self.model.set_runtime_booting();
        self.force_redraw = true;
    }

    /// Mark the TUI as initializing the canonical runtime (async).
    pub fn set_runtime_initializing(&mut self, detail: Option<String>) {
        self.model.set_runtime_initializing(detail);
        self.force_redraw = true;
    }

    /// Mark the TUI as hydrating durable session truth.
    pub fn set_runtime_hydrating_session(&mut self, detail: Option<String>) {
        self.model.set_runtime_hydrating_session(detail);
        self.force_redraw = true;
    }

    /// Mark the TUI as hydrating workspace truth.
    pub fn set_runtime_hydrating_workspace(&mut self, detail: Option<String>) {
        self.model.set_runtime_hydrating_workspace(detail);
        self.force_redraw = true;
    }

    /// Mark the TUI as hydrating execution truth.
    pub fn set_runtime_hydrating_execution(&mut self, detail: Option<String>) {
        self.model.set_runtime_hydrating_execution(detail);
        self.force_redraw = true;
    }

    /// Transition the cockpit from startup state to normal operation.
    pub fn set_runtime_ready(&mut self) {
        self.model.set_runtime_ready();
        self.force_redraw = true;
    }

    /// Surface a runtime/bridge initialization failure as visible TUI state.
    ///
    /// Never a blank screen: the shell keeps rendering header, startup
    /// error panel, composer, and footer.
    pub fn set_runtime_failed(&mut self, reason: impl Into<String>) {
        self.model.set_runtime_failed(reason);
        self.force_redraw = true;
    }

    /// Surface a typed failure (runtime / bridge / hydration / model).
    pub fn set_runtime_failed_with_kind(
        &mut self,
        kind: crate::tui::errors::TuiErrorKind,
        reason: impl Into<String>,
    ) {
        self.model.set_runtime_failed_with_kind(kind, reason);
        self.force_redraw = true;
    }

    /// True when the cockpit has left startup state.
    pub fn is_runtime_ready(&self) -> bool {
        self.model.is_runtime_ready()
    }

    /// Process bracketed paste text.
    pub fn handle_paste(&mut self, text: &str) {
        if self.navigation.active_modal == Some(ViewId::MissionCreation) {
            let safe_text = crate::tui::sanitizer::sanitize_terminal_text(text);
            self.model.modal_draft.push_str(&safe_text);
            self.model.mark_dirty();
            return;
        }

        // Bracketed paste follows the same owner precedence as key events;
        // it must never leak through a focused flag into a modal, overlay,
        // detail inspector, palette, wizard, help, or Settings screen.
        let composer_owns_input = self.is_composer_focused
            && !self.palette.is_open
            && !self.approval_modal.is_open
            && !self.overlay_manager.is_help_open
            && self.navigation.active_modal.is_none()
            && self.navigation.active_overlay.is_none()
            && self.navigation.active_detail.is_none()
            && !self.is_wizard_active()
            && self.navigation.current_screen != ScreenId::Settings;
        if composer_owns_input {
            self.composer.paste(text);
            self.model.mark_dirty();
        }
    }

    /// Process pending events from the ONE canonical TUI event ingress: the
    /// governed runtime bridge (`UnboundedReceiver<InteractionEvent>`).
    ///
    /// Every drained event is reduced through [`crate::tui::state::apply_tui_event`];
    /// there is no second channel, no legacy envelope path, no full reload.
    pub fn poll_updates(&mut self) -> usize {
        let mut count = 0;
        let max_events = crate::config::canonical::DEFAULT_TUI_MAX_EVENTS_PER_TICK;
        let mut reached_limit = false;

        if let Some(ref mut irx) = self.interaction_rx {
            while count < max_events {
                match irx.try_recv() {
                    Ok(ie) => {
                        // Synchronize approval modal with incoming approval requests and resolutions
                        match &ie {
                            InteractionEvent::ApprovalRequested {
                                request_id,
                                tool_name,
                                details,
                                risk_tier,
                                parameters_summary,
                                agent_role,
                                ..
                            } => {
                                if !self.approval_modal.is_open {
                                    self.approval_modal.open(
                                        crate::tui::model::TuiApprovalRequest {
                                            id: request_id.clone(),
                                            tool_name: if tool_name.is_empty() {
                                                "Unavailable".to_string()
                                            } else {
                                                tool_name.clone()
                                            },
                                            agent_role: agent_role
                                                .clone()
                                                .unwrap_or_else(|| "Unavailable".to_string()),
                                            justification: details.clone(),
                                            parameters_summary: parameters_summary
                                                .clone()
                                                .unwrap_or_else(|| details.clone()),
                                            risk_tier: risk_tier
                                                .clone()
                                                .unwrap_or_else(|| "Unavailable".to_string()),
                                            timestamp: chrono::Utc::now(),
                                        },
                                    );
                                    self.focus.enter_overlay();
                                }
                            }
                            InteractionEvent::ApprovalResolved { request_id, .. }
                                if self.approval_modal.is_open
                                    && self
                                        .approval_modal
                                        .current_request
                                        .as_ref()
                                        .map(|r| &r.id)
                                        == Some(request_id) =>
                            {
                                self.approval_modal.close();
                                self.focus.exit_overlay();
                                if let Some(next) =
                                    self.model.approvals.iter().find(|a| a.id != *request_id)
                                {
                                    self.approval_modal.open(next.clone());
                                    self.focus.enter_overlay();
                                }
                            }
                            _ => {}
                        }

                        if let InteractionEvent::WorkflowSnapshotUpdated { ref snapshot } = ie {
                            self.workflow_snapshot = Some((**snapshot).clone());
                        }
                        crate::tui::state::apply_tui_event(
                            &mut self.model,
                            &crate::tui::state::TuiEvent::Interaction(ie),
                        );
                        count += 1;
                    }
                    Err(_) => break,
                }
            }

            if count == max_events {
                reached_limit = true;
            }
        }

        self.has_event_backlog = reached_limit;
        count
    }

    /// Apply an interaction event directly to the live view model through the canonical reducer.
    pub fn apply_interaction_event(&mut self, event: &InteractionEvent) {
        if let InteractionEvent::WorkflowSnapshotUpdated { snapshot } = event {
            self.workflow_snapshot = Some((**snapshot).clone());
        }
        crate::tui::state::apply_tui_event(
            &mut self.model,
            &crate::tui::state::TuiEvent::Interaction(event.clone()),
        );
        self.model.mark_dirty();
        self.force_redraw = true;
    }

    /// Reconstruct complete view state from historical events upon reconnect (TUI-05).
    pub fn reconnect(&mut self, history: &[EventEnvelope]) {
        self.model.reconstruct_from_events(history);
        self.force_redraw = true;
    }

    /// Enter replay mode with specified historical events.
    /// Preserves live presentation model in `self.model`, while projecting historical state into `self.replay_model`.
    pub fn enter_replay(&mut self, history: Vec<EventEnvelope>) {
        self.replay.load_history(history);
        self.replay_model = Some(self.replay.reconstruct_current_view());
        self.force_redraw = true;
    }

    /// Exit replay mode and restore full live presentation projection.
    pub fn exit_replay(&mut self) {
        self.replay.exit();
        self.replay_model = None;
        self.model.mark_dirty();
        self.force_redraw = true;
    }

    /// Stop the event loop after the current tick.
    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// Draw the first frame synchronously. Must precede every runtime await.
    pub fn render_first_frame<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> std::io::Result<bool> {
        self.render_frame(terminal)
    }

    /// Configure bounded deadline for canonical runtime assembly.
    pub fn with_assembly_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.assembly_timeout = timeout;
        self
    }

    /// Attach the async runtime-assembly receiver from the real composition
    /// root (`main.rs` assembles `AppRuntime`; the TUI never constructs it).
    pub fn begin_assembly(&mut self, rx: UnboundedReceiver<RuntimeAssemblyOutcome>) {
        // Abort any previous in-flight attempt so retries never double-run
        // crash recovery or leave two assemblies racing.
        if let Some(ref handle) = self.assembly_abort_handle {
            handle.abort();
        }
        self.assembly_generation += 1;
        self.assembly_active_generation = self.assembly_generation;
        self.assembly_rx = Some(rx);
        self.assembly_progress_rx = None;
        self.assembly_last_stage = None;
        self.assembly_settled = false;
        self.assembly_started_at = Some(std::time::Instant::now());
        self.assembly_abort_handle = None;
        self.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    }

    /// Attach the async runtime-assembly receiver along with an explicit task abort handle.
    /// When assembly times out or fails, the abort handle cancels the background task.
    pub fn begin_assembly_with_abort_handle(
        &mut self,
        rx: UnboundedReceiver<RuntimeAssemblyOutcome>,
        abort_handle: tokio::task::AbortHandle,
    ) {
        if let Some(ref handle) = self.assembly_abort_handle {
            handle.abort();
        }
        self.assembly_generation += 1;
        self.assembly_active_generation = self.assembly_generation;
        self.assembly_rx = Some(rx);
        self.assembly_progress_rx = None;
        self.assembly_last_stage = None;
        self.assembly_settled = false;
        self.assembly_started_at = Some(std::time::Instant::now());
        self.assembly_abort_handle = Some(abort_handle);
        self.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    }

    /// Attach assembly outcome + bounded live progress + abort handle.
    ///
    /// Canonical TUI entry used by the real composition root. Progress is
    /// carried over a bounded `watch` channel (latest stage only); the
    /// outcome channel is unchanged. Retries abort the previous attempt
    /// first so crash recovery is never double-run concurrently.
    pub fn begin_assembly_with_progress(
        &mut self,
        rx: UnboundedReceiver<RuntimeAssemblyOutcome>,
        progress_rx: tokio::sync::watch::Receiver<crate::startup_progress::StartupStageUpdate>,
        abort_handle: tokio::task::AbortHandle,
    ) {
        if let Some(ref handle) = self.assembly_abort_handle {
            handle.abort();
        }
        self.assembly_generation += 1;
        self.assembly_active_generation = self.assembly_generation;
        self.assembly_rx = Some(rx);
        self.assembly_progress_rx = Some(progress_rx);
        self.assembly_last_stage = None;
        self.assembly_settled = false;
        self.assembly_started_at = Some(std::time::Instant::now());
        self.assembly_abort_handle = Some(abort_handle);
        self.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    }

    /// Last observed startup stage, if any progress arrived.
    ///
    /// `None` means no stage has emitted progress yet — distinguished from
    /// an actively-running stage in timeout diagnostics.
    pub fn last_assembly_stage(&self) -> Option<&crate::startup_progress::StartupStageUpdate> {
        self.assembly_last_stage.as_ref()
    }

    /// Current assembly generation (increments per `begin_assembly*`).
    pub fn assembly_generation(&self) -> u64 {
        self.assembly_generation
    }

    /// Drain the bounded progress channel without blocking render.
    ///
    /// Updates the live `InitializingRuntime` message from real work
    /// (e.g. `Recovering unfinished missions (3/12)…`). Returns true when
    /// the visible message changed.
    fn drain_assembly_progress(&mut self) -> bool {
        let mut changed = false;
        // `has_changed` + `borrow_and_update` coalesce bursts into the latest
        // value: slow frames never accumulate backlog.
        //
        // The watch borrow is released before mutating `self` (message
        // update) to satisfy the borrow checker.
        loop {
            let next = self.assembly_progress_rx.as_mut().and_then(|rx| {
                if rx.has_changed().unwrap_or(false) {
                    Some(rx.borrow_and_update().clone())
                } else {
                    None
                }
            });
            let Some(update) = next else { break };
            self.assembly_last_stage = Some(update.clone());
            // Only drive the visible message while still assembling; a
            // settled/retrying TUI must not resurrect stale progress.
            if !self.assembly_settled {
                self.set_runtime_initializing(Some(update.message.clone()));
            }
            changed = true;
        }
        changed
    }

    /// Format the stage-aware timeout detail.
    ///
    /// Includes the last observed stage (or the explicit no-progress state)
    /// and elapsed duration. Never includes credentials or secret values
    /// (stage updates never carry them).
    fn assembly_timeout_message(&self) -> String {
        let base = format!(
            "Runtime assembly timed out after {:.1}s (deadline exceeded)",
            self.assembly_timeout.as_secs_f64()
        );
        match self.assembly_last_stage.as_ref() {
            Some(update) => format!(
                "{base}; last stage '{}' (seq {}): {}",
                update.stage, update.seq, update.message
            ),
            None => format!("{base}; no startup stage reported progress before the deadline"),
        }
    }

    /// Poll async runtime assembly without blocking render.
    ///
    /// On `Ready`: walks the phased hydration
    /// (`HydratingSession → HydratingWorkspace → HydratingExecution → Ready`),
    /// drawing each step before its work so partial data stays visible.
    /// On failure or deadline exceeded: records a visible typed error (never blank).
    ///
    /// Stage-aware behavior:
    /// - Live progress updates refresh the initializing message every frame.
    /// - The total deadline is unchanged (30s default); per-stage hydration
    ///   and bridge attachment keep their own 10s bounds inside `attach`.
    /// - On timeout the background task is aborted (cancelling timed-out
    ///   work) and the error names the last observed stage.
    /// - A `Ready` arriving for a superseded generation (stale completion
    ///   from a timed-out attempt) is ignored and never attaches.
    pub async fn poll_runtime<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> bool {
        if self.assembly_settled {
            return false;
        }
        let active_generation = self.assembly_active_generation;

        // Drain live progress first so the frame shows what assembly is
        // actually doing (or that nothing has reported yet).
        if self.drain_assembly_progress() {
            let _ = self.render_frame(terminal);
        }

        // Bounded startup deadline: a stalled runtime assembly must transition
        // to visible Failed state rather than hanging indefinitely.
        if let Some(started_at) = self.assembly_started_at {
            if started_at.elapsed() >= self.assembly_timeout {
                self.assembly_settled = true;
                if let Some(ref handle) = self.assembly_abort_handle {
                    handle.abort();
                }
                let msg = self.assembly_timeout_message();
                tracing::error!(
                    stage = self
                        .assembly_last_stage
                        .as_ref()
                        .map(|u| u.stage.as_str())
                        .unwrap_or("startup.unknown"),
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "runtime assembly deadline exceeded"
                );
                let err = TuiError::runtime(msg);
                self.binding.record_failure(err.clone());
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.render_frame(terminal);
                return true;
            }
        }

        let outcome = match self.assembly_rx.as_mut() {
            Some(rx) => match rx.try_recv() {
                Ok(o) => Some(o),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    Some(RuntimeAssemblyOutcome::Failed(TuiError::runtime(
                        "runtime initialization task terminated without a result",
                    )))
                }
            },
            None => None,
        };
        let Some(outcome) = outcome else {
            return false;
        };
        // Stale-completion guard: ignore outcomes from superseded attempts.
        if active_generation != self.assembly_active_generation {
            tracing::warn!("ignoring stale runtime assembly outcome from superseded attempt");
            return false;
        }
        match outcome {
            RuntimeAssemblyOutcome::Ready(rt) => {
                // A late Ready arriving after the deadline settled the attempt
                // must never attach (generation already superseded or settled).
                if self.assembly_settled {
                    tracing::warn!(
                        "ignoring late runtime Ready after assembly settled (stale completion)"
                    );
                    return false;
                }
                self.attach_ready_runtime(terminal, rt).await;
                true
            }
            RuntimeAssemblyOutcome::Failed(err) => {
                self.assembly_settled = true;
                if let Some(ref handle) = self.assembly_abort_handle {
                    handle.abort();
                }
                self.binding.record_failure(err.clone());
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.render_frame(terminal);
                true
            }
        }
    }

    async fn attach_ready_runtime<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        rt: Arc<crate::runtime::AppRuntime>,
    ) {
        self.assembly_settled = true;
        if self.resolved_config.is_none() {
            let cfg = rt.config();
            self.resolved_config = Some(cfg.clone());
            self.settings_draft = Some(cfg.app_config.clone());
            let channel = crate::deployment::DeploymentChannel::current();
            let creds = crate::runtime_authorities::resolve_runtime_credentials(
                &cfg.workspace_root,
                channel,
            );
            self.model.credential_status = if creds.api_key.is_some() {
                crate::tui::model::TuiCredentialStatus::Configured
            } else {
                crate::tui::model::TuiCredentialStatus::NotConfigured
            };
        }
        // Announce workspace hydration (stage-tagged for diagnostics):
        tracing::info!(
            stage = crate::startup_progress::stage::TUI_WORKSPACE_HYDRATION,
            status = "started",
            "workspace hydration started"
        );
        let hydration_started = std::time::Instant::now();
        self.set_runtime_hydrating_workspace(Some("Loading workspace state…".to_string()));
        let _ = self.render_frame(terminal);
        let hydrate_res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.hydrate_from_runtime(&rt),
        )
        .await;
        if hydrate_res.is_err() {
            tracing::warn!(
                stage = crate::startup_progress::stage::TUI_WORKSPACE_HYDRATION,
                status = "warning",
                elapsed_ms = hydration_started.elapsed().as_millis() as u64,
                "Workspace state hydration timed out after 10s; continuing in degraded mode"
            );
        } else {
            tracing::info!(
                stage = crate::startup_progress::stage::TUI_WORKSPACE_HYDRATION,
                status = "success",
                elapsed_ms = hydration_started.elapsed().as_millis() as u64,
                "workspace hydration completed"
            );
        }

        // Announce bridge connect next:
        tracing::info!(
            stage = crate::startup_progress::stage::TUI_BRIDGE_ATTACHMENT,
            status = "started",
            "cockpit bridge attachment started"
        );
        let bridge_started = std::time::Instant::now();
        self.set_runtime_hydrating_execution(Some("Connecting cockpit bridge…".to_string()));
        let _ = self.render_frame(terminal);

        let attach_res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.binding.attach_runtime(rt.clone(), None),
        )
        .await;

        match attach_res {
            Ok(Ok(())) => {
                tracing::info!(
                    stage = crate::startup_progress::stage::TUI_BRIDGE_ATTACHMENT,
                    status = "success",
                    elapsed_ms = bridge_started.elapsed().as_millis() as u64,
                    "cockpit bridge attached"
                );
                // Move bridge channels onto the live (already hydrated)
                // projection. Only channels change — model, navigation,
                // composer, and theme are preserved.
                if let Some(tx) = self.binding.bridge_sender() {
                    self.bridge_tx = Some(tx);
                }
                if let Some(irx) = self.binding.take_interaction_receiver() {
                    self.interaction_rx = Some(irx);
                }
                self.set_runtime_ready();
                let _ = self.render_frame(terminal);
            }
            Ok(Err(err)) => {
                tracing::error!(
                    stage = crate::startup_progress::stage::TUI_BRIDGE_ATTACHMENT,
                    status = "failure",
                    elapsed_ms = bridge_started.elapsed().as_millis() as u64,
                    "cockpit bridge attachment failed: {}",
                    err.message
                );
                self.binding.record_failure(err.clone());
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.render_frame(terminal);
            }
            Err(_) => {
                tracing::error!(
                    stage = crate::startup_progress::stage::TUI_BRIDGE_ATTACHMENT,
                    status = "failure",
                    elapsed_ms = bridge_started.elapsed().as_millis() as u64,
                    "cockpit bridge attachment timed out after 10s"
                );
                let err = TuiError::bridge("Connecting cockpit bridge timed out after 10s");
                self.binding.record_failure(err.clone());
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.render_frame(terminal);
            }
        }
        // Keep the canonical runtime alive via the binding.
        let _ = self.binding.runtime();
    }

    /// Supervise the bridge task: surface termination as visible error.
    pub fn supervise_bridge(&mut self) -> bool {
        if let Some(err) = self.binding.poll_bridge_supervision() {
            if self.is_runtime_ready() {
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                return true;
            }
        }
        false
    }

    /// Whether the setup wizard is currently active as an overlay or modal dialog.
    pub fn is_wizard_active(&self) -> bool {
        self.setup_wizard.is_some()
            || self.navigation.active_modal == Some(ViewId::SetupWizard)
            || self.navigation.active_overlay == Some(ViewId::SetupWizard)
    }

    /// Classify the next key under the single input-priority authority
    /// (`keymap::resolve_key`): replay → dialog → palette → help → wizard →
    /// palette-chord → interrupt → composer → route.
    ///
    /// The classification is CONSUMED by `handle_key`, never discarded.
    pub fn classify_key(&mut self, key: KeyEvent) -> KeyAction {
        let is_wiz = self.setup_wizard.is_some()
            || self.navigation.active_overlay == Some(ViewId::SetupWizard);
        let is_dlg = self.approval_modal.is_open || self.navigation.active_modal.is_some();
        let is_help = self.overlay_manager.is_help_open
            || (self.navigation.active_overlay.is_some()
                && self.navigation.active_modal.is_none()
                && !is_wiz);

        let is_settings = self.navigation.current_screen == ScreenId::Settings;
        let is_composer = self.is_composer_focused
            && self.navigation.active_modal.is_none()
            && self.navigation.active_overlay.is_none()
            && self.navigation.active_detail.is_none()
            && !is_wiz
            && !is_settings;

        let ctx = KeyContext::new(
            self.replay.is_active,
            is_dlg,
            self.palette.is_open,
            is_help,
            is_wiz,
            is_composer,
        )
        .with_decision(self.model.has_pending_decision());
        resolve_key(key, ctx)
    }

    /// Exit chord check (q / Ctrl+C / Ctrl+D).
    pub fn should_exit(&self, key: KeyEvent) -> bool {
        let no_dialog = !self.approval_modal.is_open
            && self.navigation.active_modal.is_none()
            && self.navigation.active_overlay.is_none()
            && self.navigation.active_detail.is_none()
            && !self.palette.is_open
            && !self.overlay_manager.is_help_open
            && !self.is_wizard_active();
        match key.code {
            KeyCode::Char('q') => {
                key.modifiers.is_empty() && no_dialog && !self.is_composer_focused
            }
            KeyCode::Char('c') | KeyCode::Char('d') => {
                key.modifiers.contains(KeyModifiers::CONTROL)
                    && no_dialog
                    && !self.composer_owns_cancel()
            }
            _ => false,
        }
    }

    /// Whether the composer currently owns cancel (Ctrl+C clears instead of exits).
    pub fn composer_owns_cancel(&self) -> bool {
        self.model.has_active_animation()
            || (self.is_composer_focused && !self.composer.text().is_empty())
    }

    /// Handle keyboard input through the single input-priority authority.
    ///
    /// `classify_key` makes the ONE ownership decision
    /// (replay → dialog → palette → help → wizard → palette-chord →
    /// interrupt → decision → composer → route); each owner below handles keys and
    /// translates them to `ApplicationAction`s. No second matcher exists.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        match self.classify_key(key) {
            KeyAction::Replay => {
                // Read-only scrub: mutation commands are blocked (D-19, T-11-20).
                if self.replay.handle_key(key) {
                    if self.replay.is_active {
                        self.replay_model = Some(self.replay.reconstruct_current_view());
                        if let Some(ref mut m) = self.replay_model {
                            m.mark_dirty();
                        }
                    } else {
                        self.exit_replay();
                    }
                }
                None
            }
            KeyAction::Dialog => self.handle_dialog_key(key),
            KeyAction::Palette => self.handle_palette_key(key),
            KeyAction::Help => self.handle_help_key(key),
            KeyAction::Wizard => self.handle_wizard_key(key),
            KeyAction::OpenPalette => {
                self.overlay_manager.close_help();
                self.navigation.close_overlay();
                self.palette.open();
                self.focus.enter_overlay();
                self.model.mark_dirty();
                None
            }
            KeyAction::Decision => self.handle_decision_key(key),
            KeyAction::Exit => self.handle_cancel_key(key),
            KeyAction::Composer => self.handle_composer_key(key),
            KeyAction::Route | KeyAction::Global => self.handle_route_key(key),
        }
    }

    /// Active interactive decision prompt owns the key (↑/↓, 1-9, Enter, Esc, Tab/c for custom mode).
    fn handle_decision_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        let decision = self.model.pending_decision.as_mut()?;

        // Viewport navigation is always permitted, even while awaiting submission response
        if key.code == KeyCode::PageUp {
            let step = self.model.page_step();
            self.model.scroll_up(step);
            return None;
        }
        if key.code == KeyCode::PageDown {
            let step = self.model.page_step();
            self.model.scroll_down(step);
            return None;
        }
        if key.code == KeyCode::Home {
            self.model.scroll_to_top();
            return None;
        }
        if key.code == KeyCode::End {
            self.model.scroll_to_bottom();
            return None;
        }

        // If a submission is actively in progress, block input to prevent double submission
        if decision.state.is_submitting() {
            return None;
        }

        if decision.is_custom_mode {
            match key.code {
                KeyCode::Esc => {
                    decision.exit_custom_mode();
                    self.model.mark_dirty();
                }
                KeyCode::Enter => {
                    let trimmed = decision.custom_input.trim().to_string();
                    if trimmed.is_empty() {
                        self.model.add_conversation_item(
                            crate::tui::conversation::TuiConversationItem::Error {
                                message: "Custom answer/feedback cannot be empty. Please provide details or press Esc to return.".to_string(),
                                timestamp: chrono::Utc::now(),
                            },
                        );
                        self.model.mark_dirty();
                        return None;
                    }

                    let selected_opt = decision.selected_option().cloned();
                    let resp = crate::interaction::decision::InteractiveDecisionResponse {
                        decision_id: decision.id.clone(),
                        session_id: decision.session_id.clone(),
                        execution_id: decision.execution_id.clone(),
                        selected_option_id: selected_opt.as_ref().map(|o| o.id.clone()),
                        selected_option_ids: Vec::new(),
                        custom_answer: Some(trimmed),
                        cancelled: false,
                    };

                    decision.state = crate::interaction::decision::DecisionState::Submitting;
                    self.model.mark_dirty();
                    self.send_or_fail(
                        "submit decision",
                        ApplicationAction::DecisionResponseSubmitted(resp),
                    );
                }
                KeyCode::Backspace => {
                    decision.custom_input.pop();
                    self.model.mark_dirty();
                }
                KeyCode::Char(c) => {
                    decision.custom_input.push(c);
                    self.model.mark_dirty();
                }
                _ => {}
            }
            return None;
        }

        match key.code {
            KeyCode::Tab | KeyCode::Char('c') if decision.allow_custom_answer => {
                decision.enter_custom_mode();
                self.model.mark_dirty();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                decision.select_prev();
                self.model.mark_dirty();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                decision.select_next();
                self.model.mark_dirty();
            }
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < decision.options.len() {
                    decision.selected_index = idx;
                    self.model.mark_dirty();
                }
            }
            KeyCode::Esc => {
                let resp = crate::interaction::decision::InteractiveDecisionResponse {
                    decision_id: decision.id.clone(),
                    session_id: decision.session_id.clone(),
                    execution_id: decision.execution_id.clone(),
                    selected_option_id: None,
                    selected_option_ids: Vec::new(),
                    custom_answer: None,
                    cancelled: true,
                };
                decision.state = crate::interaction::decision::DecisionState::Submitting;
                self.model.mark_dirty();
                self.send_or_fail(
                    "cancel decision",
                    ApplicationAction::DecisionResponseSubmitted(resp),
                );
            }
            KeyCode::Enter => {
                let selected_opt = decision.selected_option().cloned();

                // If "revise" is selected and custom answer is allowed, transition into custom mode
                // so the user can enter their revision instructions instead of submitting without feedback.
                if let Some(ref opt) = selected_opt {
                    if (opt.id == "revise" || opt.label.to_lowercase().contains("revise"))
                        && decision.allow_custom_answer
                    {
                        decision.enter_custom_mode();
                        self.model.mark_dirty();
                        return None;
                    }
                }

                let resp = crate::interaction::decision::InteractiveDecisionResponse {
                    decision_id: decision.id.clone(),
                    session_id: decision.session_id.clone(),
                    execution_id: decision.execution_id.clone(),
                    selected_option_id: selected_opt.as_ref().map(|o| o.id.clone()),
                    selected_option_ids: Vec::new(),
                    custom_answer: None,
                    cancelled: false,
                };
                decision.state = crate::interaction::decision::DecisionState::Submitting;
                self.model.mark_dirty();
                self.send_or_fail(
                    "submit decision",
                    ApplicationAction::DecisionResponseSubmitted(resp),
                );
            }
            _ => {}
        }
        None
    }

    /// Active approval dialog owns the key (D-17).
    ///
    /// Single canonical approval path (§8, §27): tool approvals resolve
    /// through the bridge (`ApplicationAction::ApprovalDecision` →
    /// `ApprovalCoordinator`), never via a parallel dispatch.
    fn handle_dialog_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        if self.focus.current() != FocusTarget::Overlay {
            self.focus.enter_overlay();
        }
        if self.approval_modal.is_open {
            let request_id = self
                .approval_modal
                .current_request
                .as_ref()
                .map(|r| r.id.clone());
            if let Some(decision) = self.approval_modal.handle_key(key) {
                self.model.mark_dirty();
                if let Some(approval_id) = request_id {
                    let sent = self.send_or_fail(
                        "resolve approval",
                        ApplicationAction::ApprovalDecision {
                            request_id: approval_id.clone(),
                            decision,
                        },
                    );
                    if sent {
                        self.model.approvals.retain(|a| a.id != approval_id);
                        self.focus.exit_overlay();
                    } else {
                        self.model.set_error(format!(
                            "Failed to dispatch approval decision for {approval_id}"
                        ));
                    }
                } else {
                    self.focus.exit_overlay();
                }
            }
            return None;
        }

        if let Some(modal_id) = self.navigation.active_modal {
            match modal_id {
                ViewId::ApprovalsQueue => {
                    match key.code {
                        KeyCode::Esc => {
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            if !self.model.approvals.is_empty() {
                                let approvals = std::mem::take(&mut self.model.approvals);
                                let mut remaining = Vec::new();
                                let mut sent_count = 0;
                                let mut fail_count = 0;
                                for a in approvals {
                                    let sent = self.send_or_fail(
                                        "resolve approval",
                                        ApplicationAction::ApprovalDecision {
                                            request_id: a.id.clone(),
                                            decision:
                                                crate::tui::approval::ApprovalDecision::ApproveOnce,
                                        },
                                    );
                                    if sent {
                                        sent_count += 1;
                                    } else {
                                        fail_count += 1;
                                        remaining.push(a);
                                    }
                                }
                                self.model.approvals = remaining;
                                if fail_count == 0 {
                                    self.navigation.close_modal();
                                    self.focus.exit_overlay();
                                } else {
                                    self.model.set_error(format!(
                                        "Partial approval dispatch: {sent_count} sent, {fail_count} failed to dispatch"
                                    ));
                                }
                                self.model.mark_dirty();
                            }
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            if !self.model.approvals.is_empty() {
                                let approvals = std::mem::take(&mut self.model.approvals);
                                let mut remaining = Vec::new();
                                let mut sent_count = 0;
                                let mut fail_count = 0;
                                for a in approvals {
                                    let sent = self.send_or_fail(
                                        "reject approval",
                                        ApplicationAction::ApprovalDecision {
                                            request_id: a.id.clone(),
                                            decision:
                                                crate::tui::approval::ApprovalDecision::Reject,
                                        },
                                    );
                                    if sent {
                                        sent_count += 1;
                                    } else {
                                        fail_count += 1;
                                        remaining.push(a);
                                    }
                                }
                                self.model.approvals = remaining;
                                if fail_count == 0 {
                                    self.navigation.close_modal();
                                    self.focus.exit_overlay();
                                } else {
                                    self.model.set_error(format!(
                                        "Partial rejection dispatch: {sent_count} sent, {fail_count} failed to dispatch"
                                    ));
                                }
                                self.model.mark_dirty();
                            }
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
                ViewId::RollbackInspector => {
                    match key.code {
                        KeyCode::Esc => {
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        KeyCode::Enter => {
                            if let Some(target) = self.model.valid_rollback_target() {
                                let target_str = target.to_string();
                                if self.model.rollback_pending {
                                    self.model.set_error(
                                        "Checkpoint restore is still pending runtime confirmation."
                                            .to_string(),
                                    );
                                } else {
                                    let sent = self.send_or_fail(
                                        "confirm rollback",
                                        ApplicationAction::CheckpointRestoreRequested {
                                            checkpoint_id: target_str.clone(),
                                        },
                                    );
                                    if sent {
                                        self.model.rollback_pending = true;
                                        self.model.add_log(
                                            "INFO",
                                            format!("Checkpoint restore for {target_str} submitted; awaiting runtime confirmation"),
                                            "checkpoint",
                                        );
                                    } else {
                                        self.model.set_error(format!(
                                            "Failed to dispatch checkpoint restore to {target_str}"
                                        ));
                                    }
                                }
                            } else {
                                self.model.set_error(
                                    "Rollback unavailable: no valid checkpoint target found."
                                        .to_string(),
                                );
                            }
                            self.model.mark_dirty();
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
                ViewId::QuarantineManager => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            let gen_id = self.model.quarantine_generation.saturating_add(1);
                            self.model.quarantine_generation = gen_id;
                            self.model.quarantine_state =
                                crate::tui::model::QuarantineProjectionState::Loading;
                            if !self.send_or_fail(
                                "refresh quarantine",
                                ApplicationAction::QuarantineRefreshRequested {
                                    generation: gen_id,
                                },
                            ) {
                                self.model.set_quarantine_failed(
                                    "Quarantine query was not dispatched; retry when the runtime bridge is available.",
                                );
                            }
                            self.model.mark_dirty();
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
                ViewId::StartupRecovery => {
                    match key.code {
                        KeyCode::Esc => {
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            if self.send_or_fail(
                                "resume session",
                                ApplicationAction::SlashCommandSubmitted {
                                    command: "resume".to_string(),
                                    args: vec![],
                                },
                            ) {
                                self.model.enter_active_session();
                                self.navigation.close_modal();
                                self.focus.exit_overlay();
                            } else {
                                self.model.set_error(
                                    "Resume was not submitted; recovery remains available for retry."
                                        .to_string(),
                                );
                            }
                            self.model.mark_dirty();
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            if self.send_or_fail(
                                "new session",
                                ApplicationAction::ClearSessionRequested,
                            ) {
                                self.navigation.close_modal();
                                self.focus.exit_overlay();
                            } else {
                                self.model.set_error(
                                    "New session was not submitted; recovery remains available for retry."
                                        .to_string(),
                                );
                            }
                            self.model.mark_dirty();
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
                ViewId::SetupWizard => {
                    return self.handle_wizard_key(key);
                }
                ViewId::MissionCreation => {
                    match key.code {
                        KeyCode::Esc => {
                            self.model.modal_draft.clear();
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        KeyCode::Enter => {
                            let text = self.model.modal_draft.trim().to_string();
                            if !text.is_empty() {
                                let draft_text = text.clone();
                                let parsed = crate::interaction::mentions::MentionParser::parse(
                                    &draft_text,
                                    std::path::Path::new(&self.model.workspace_path),
                                );
                                let sent = self.send_or_fail(
                                    "initialize mission",
                                    ApplicationAction::UserTextSubmitted(parsed),
                                );
                                if sent {
                                    self.model.enter_active_session();
                                    self.model.objective = draft_text.clone();
                                    self.model.begin_request(&draft_text);
                                    self.model.modal_draft.clear();
                                    self.navigation.close_modal();
                                    self.focus.exit_overlay();
                                } else {
                                    // Keep the draft and dialog intact for a safe retry.
                                    self.model.set_error(
                                        "Mission initialization failed: runtime bridge unavailable",
                                    );
                                }
                                self.model.mark_dirty();
                            }
                        }
                        KeyCode::Backspace => {
                            self.model.modal_draft.pop();
                            self.model.mark_dirty();
                        }
                        KeyCode::Char(c) => {
                            self.model.modal_draft.push(c);
                            self.model.mark_dirty();
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
                _ => {
                    match key.code {
                        KeyCode::Esc => {
                            self.navigation.close_modal();
                            self.focus.exit_overlay();
                            self.model.mark_dirty();
                        }
                        _ => {
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
            }
        }

        None
    }

    /// Open command palette owns the key.
    fn handle_palette_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        if self.focus.current() != FocusTarget::Overlay {
            self.focus.enter_overlay();
        }
        if let Some(action) = self.palette.handle_key(key) {
            self.focus.exit_overlay();
            match action {
                PaletteActionV2::NavigateView(view_id) => {
                    self.navigate_to_view(view_id);
                }
                PaletteActionV2::ExecuteCommand(cmd) => {
                    self.model.enter_active_session();
                    self.model.mark_dirty();
                    match &cmd {
                        RuntimeCommand::PauseMission { id } => {
                            let mission_id = if id == "current" {
                                self.model.mission_id.clone().unwrap_or_default()
                            } else {
                                id.clone()
                            };
                            self.send_or_fail(
                                "pause mission",
                                ApplicationAction::MissionPauseRequested { mission_id },
                            );
                        }
                        RuntimeCommand::ResumeMission { id } => {
                            let mission_id = if id == "current" {
                                self.model.mission_id.clone().unwrap_or_default()
                            } else {
                                id.clone()
                            };
                            self.send_or_fail(
                                "resume mission",
                                ApplicationAction::MissionResumeRequested { mission_id },
                            );
                        }
                        RuntimeCommand::CancelMission { .. } => {
                            self.model.settle_request();
                            self.send_or_fail("cancel", ApplicationAction::CancelRequested);
                        }
                        RuntimeCommand::RunDoctor { .. } => {
                            self.navigation.navigate_to(ScreenId::Doctor);
                            self.send_or_fail(
                                "doctor",
                                ApplicationAction::SlashCommandSubmitted {
                                    command: "doctor".to_string(),
                                    args: vec![],
                                },
                            );
                        }
                        RuntimeCommand::Version { .. } => {
                            self.send_or_fail(
                                "version",
                                ApplicationAction::SlashCommandSubmitted {
                                    command: "version".to_string(),
                                    args: vec![],
                                },
                            );
                        }
                        other => {
                            self.model
                                .fail_request(format!("Unsupported palette command: {other:?}"));
                            self.model.add_conversation_item(
                                crate::tui::conversation::TuiConversationItem::Error {
                                    message: format!(
                                        "Palette command '{other:?}' is not supported from the cockpit palette."
                                    ),
                                    timestamp: chrono::Utc::now(),
                                },
                            );
                            self.model.add_log(
                                "WARN",
                                format!("Unsupported palette command: {other:?}"),
                                "tui",
                            );
                        }
                    }
                    return None;
                }
                PaletteActionV2::Action(act_str) => {
                    self.model.mark_dirty();
                    if act_str == "toggle_theme" {
                        self.theme_mode = self.theme_mode.cycle();
                    } else if let Some(stripped) = act_str.strip_prefix('/') {
                        self.model.enter_active_session();
                        let action = ApplicationAction::SlashCommandSubmitted {
                            command: stripped.to_string(),
                            args: Vec::new(),
                        };
                        // Palette slash commands are silent requests like
                        // composer ones: never Thinking, settled by the
                        // terminal CommandOutput/Error event.
                        self.model.begin_request_silent();
                        self.send_or_fail("/palette-command", action);
                    } else {
                        self.model.add_conversation_item(
                            crate::tui::conversation::TuiConversationItem::Error {
                                message: format!("Unknown palette action '{act_str}'"),
                                timestamp: chrono::Utc::now(),
                            },
                        );
                    }
                }
                PaletteActionV2::Close => {
                    self.model.mark_dirty();
                }
            }
        }
        None
    }

    /// Open help overlay owns the key: Esc/?/q closes, everything else is
    /// swallowed so the overlay keeps focus.
    fn handle_help_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        if self.focus.current() != FocusTarget::Overlay {
            self.focus.enter_overlay();
        }
        if key.code == KeyCode::Esc
            || key.code == KeyCode::Char('?')
            || key.code == KeyCode::Char('q')
        {
            self.overlay_manager.close_help();
            self.navigation.close_overlay();
            self.navigation.close_modal();
            self.focus.exit_overlay();
            self.model.mark_dirty();
        }
        None
    }

    /// Setup wizard dialog owns the key (View 22 - ModalDialog).
    /// Completion persists configuration through the wizard's canonical
    /// config path; cancellation leaves durable state untouched.
    fn handle_wizard_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        if self.focus.current() != FocusTarget::Overlay {
            self.focus.enter_overlay();
        }
        if self.setup_wizard.is_none() {
            let root = if self.model.workspace_path.trim().is_empty() {
                PathBuf::from(".")
            } else {
                PathBuf::from(&self.model.workspace_path)
            };
            self.setup_wizard = Some(SetupWizardScreen::new(root));
        }
        if let Some(ref mut wizard) = self.setup_wizard {
            let outcome = wizard.handle_key(key);
            self.model.mark_dirty();
            match outcome {
                WizardOutcome::Completed => {
                    if let Err(e) = wizard.persist_configuration() {
                        wizard.status_message = Some(format!("Failed to persist config: {e}"));
                    } else {
                        self.navigation.close_overlay();
                        self.navigation.close_modal();
                        self.setup_wizard = None;
                        self.focus.exit_overlay();
                        self.model.add_log(
                            "INFO",
                            "Setup Wizard completed and configuration persisted",
                            "wizard",
                        );
                        self.model.mark_dirty();
                    }
                }
                WizardOutcome::Cancelled => {
                    self.navigation.close_overlay();
                    self.navigation.close_modal();
                    self.setup_wizard = None;
                    self.focus.exit_overlay();
                    self.model.mark_dirty();
                }
                WizardOutcome::Error(_msg) => {
                    // Error already shown in wizard status message
                }
                _ => {}
            }
        }
        None
    }

    /// Interrupt / exit chord (Ctrl+C / Ctrl+D).
    ///
    /// Single input authority (replay → dialog → palette → help → wizard →
    /// interrupt → composer → route):
    /// 1. If composer has text: clears composer text.
    /// 2. If an active operation/request/animation is in flight: cancels it via the bridge.
    /// 3. Otherwise (composer empty, no operation active): stops the application cleanly.
    fn handle_cancel_key(&mut self, _key: KeyEvent) -> Option<RuntimeCommand> {
        // 1. If composer has text: Ctrl+C clears the composer text
        if self.is_composer_focused && !self.composer.text().is_empty() {
            self.composer.clear();
            self.model.mark_dirty();
            return None;
        }

        // 2. Cancellation flows through the single bridge path (§8)
        if self.bridge_tx.is_some() {
            self.model.settle_request();
            self.send_or_fail("cancel", ApplicationAction::CancelRequested);
            return None;
        }

        if let Some(mid) = self.model.mission_id.clone() {
            self.model.settle_request();
            return Some(RuntimeCommand::CancelMission {
                id: mid,
                reason: Some("Cancelled via Ctrl+C".to_string()),
            });
        }

        // 3. Otherwise (no bridge / standalone, composer empty): stops the application cleanly
        self.stop();
        None
    }

    /// Focused composer owns the key: viewport scroll chords, focus chords,
    /// discovery selection, then the composer's own editing. Submission
    /// routes to `route_composer_submit`; the composer never sees runtime
    /// internals and the application never re-implements editing.
    fn handle_composer_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        if key.code == KeyCode::PageUp {
            let step = self.model.page_step();
            self.model.scroll_up(step);
            return None;
        }
        if key.code == KeyCode::PageDown {
            let step = self.model.page_step();
            self.model.scroll_down(step);
            return None;
        }
        if key.code == KeyCode::Home && self.composer.text().is_empty() {
            self.model.scroll_to_top();
            return None;
        }
        if key.code == KeyCode::End && self.composer.text().is_empty() {
            self.model.scroll_to_bottom();
            return None;
        }
        // Ctrl+Up / Ctrl+Down scrolls the conversation without leaving
        // the composer or hijacking Up/Down history semantics.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && (key.code == KeyCode::Up || key.code == KeyCode::Down)
        {
            if key.code == KeyCode::Up {
                self.model.scroll_up(3);
            } else {
                self.model.scroll_down(3);
            }
            return None;
        }

        if key.code == KeyCode::Esc {
            if self.composer.is_autocomplete_open() {
                self.composer.close_autocomplete();
            } else {
                self.unfocus_composer();
            }
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::Tab && !self.composer.is_autocomplete_open() {
            let has_rail = self.has_context_panel();
            self.focus.cycle_next(has_rail);
            self.is_composer_focused = self.focus.current() == FocusTarget::Composer;
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::BackTab {
            let has_rail = self.has_context_panel();
            self.focus.cycle_prev(has_rail);
            self.is_composer_focused = self.focus.current() == FocusTarget::Composer;
            self.model.mark_dirty();
            return None;
        }

        if self.model.lifecycle.stage == TuiLifecycleStage::DiscoveryRequired
            && self.composer.text().is_empty()
            && !self.composer.is_autocomplete_open()
        {
            if key.code == KeyCode::Up {
                self.model.lifecycle.select_prev_option();
                self.model.mark_dirty();
                return None;
            } else if key.code == KeyCode::Down {
                self.model.lifecycle.select_next_option();
                self.model.mark_dirty();
                return None;
            }
        }

        let action = self.composer.handle_key(key);
        self.model.mark_dirty();

        match action {
            ComposerAction::Submit(mut text) => {
                if text.trim().is_empty()
                    && self.model.lifecycle.stage == TuiLifecycleStage::DiscoveryRequired
                {
                    if let Some(opt) = self.model.lifecycle.selected_option() {
                        text = opt.to_string();
                    }
                }

                if self.model.lifecycle.stage == TuiLifecycleStage::DiscoveryRequired
                    && !text.starts_with('/')
                {
                    let active_q_opt = self
                        .model
                        .lifecycle
                        .pending_discovery_questions
                        .first()
                        .cloned();
                    if let Some(active_q) = active_q_opt {
                        if !active_q.options.is_empty() && !active_q.allow_freeform {
                            let matches = active_q
                                .options
                                .iter()
                                .any(|o| o.trim().eq_ignore_ascii_case(text.trim()) || o == &text);
                            if !matches {
                                self.model
                                    .add_conversation_item(TuiConversationItem::Error {
                                        message: format!(
                                            "Invalid selection: '{}'. Must select one of: {}",
                                            text,
                                            active_q.options.join(", ")
                                        ),
                                        timestamp: chrono::Utc::now(),
                                    });
                                return None;
                            }
                        }

                        self.model.enter_active_session();
                        let conv_len = self.model.conversation.len();
                        self.model.add_conversation_item(TuiConversationItem::User {
                            id: uuid::Uuid::now_v7().to_string(),
                            sequence: conv_len as u64 + 1,
                            text: text.clone(),
                            mentions: Vec::new(),
                            timestamp: chrono::Utc::now(),
                        });
                        self.model.begin_request_silent();
                        self.send_or_fail(
                            "discovery-answer",
                            ApplicationAction::QuestionAnswerSubmitted {
                                session_id: self.model.lifecycle.session_id.clone(),
                                question_id: active_q.question_id.clone(),
                                answer: text,
                            },
                        );
                        return None;
                    }
                }

                // Input routing FIRST: classify before owning any
                // activity state. Slash commands must never enter
                // model-thinking UI merely because input was submitted.
                let parser = crate::interaction::parser::InteractionParser::default();
                let has_active_mission = self.model.mission_id.is_some();
                // §52: the composer routes input against the canonical
                // lifecycle projection — never a hardcoded Idle that
                // would misroute governance-gated input.
                let prompt_state = composer_prompt_state(&self.model.lifecycle.stage);
                let Some(routed) = parser.parse(
                    &text,
                    std::path::Path::new(&self.model.workspace_path),
                    prompt_state,
                    None,
                    has_active_mission,
                ) else {
                    // Nothing to route (empty/whitespace): no timeline
                    // item, no activity, no request. Never fake work.
                    return None;
                };
                self.model.enter_active_session();
                // Record user input in the conversation timeline. Both
                // slash commands and natural language appear as `You`;
                // the runtime outcome follows as an `M31A` response.
                self.model.add_conversation_item(TuiConversationItem::User {
                    id: uuid::Uuid::now_v7().to_string(),
                    sequence: self.model.conversation.len() as u64 + 1,
                    text: text.clone(),
                    mentions: Vec::new(),
                    timestamp: chrono::Utc::now(),
                });

                match routed {
                    ApplicationAction::SettingsRequested { category } => {
                        if let Some(c) = category {
                            if let Some(cat) =
                                crate::tui::surface::settings::SettingsCategory::from_str_relaxed(
                                    &c,
                                )
                            {
                                self.settings_state.select_category(cat);
                            }
                        }
                        self.navigation.navigate_to(ScreenId::Settings);
                        self.navigation
                            .navigate_to_view(crate::tui::registry::ViewId::SettingsConfig);
                        self.normalize_focus();
                        self.unfocus_composer();
                        self.model.settle_request();
                        None
                    }
                    ApplicationAction::DoctorRequested { category } => {
                        self.navigation.navigate_to(ScreenId::Doctor);
                        self.navigation
                            .navigate_to_view(crate::tui::registry::ViewId::DoctorDiagnostics);
                        self.unfocus_composer();
                        self.model.settle_request();
                        self.send_or_fail(
                            "doctor",
                            ApplicationAction::DoctorRequested { category },
                        );
                        None
                    }
                    ApplicationAction::DiffRequested => {
                        self.navigation.navigate_to(ScreenId::Git);
                        // Read-only inspection: no model activity owned.
                        self.send_or_fail("diff", ApplicationAction::DiffRequested);
                        None
                    }
                    ApplicationAction::ClearRequested => {
                        // Local projection cleared AND canonical
                        // durable clear requested: one honest path.
                        // Clearing settles any activity (nothing is
                        // "working" on an empty timeline).
                        self.model.conversation.clear();
                        self.model.settle_request();
                        self.send_or_fail("clear", ApplicationAction::ClearRequested);
                        None
                    }
                    ApplicationAction::ClearSessionRequested => {
                        self.model.settle_request();
                        self.send_or_fail(
                            "clear-session",
                            ApplicationAction::ClearSessionRequested,
                        );
                        None
                    }
                    ApplicationAction::CancelRequested => {
                        // Single canonical path (§8): bridge owns
                        // cancellation; direct dispatch only when the
                        // bridge is absent (fail-closed otherwise).
                        // Cancellation itself owns no thinking state.
                        self.model.settle_request();
                        if self.bridge_tx.is_some() {
                            self.send_or_fail("cancel", ApplicationAction::CancelRequested);
                            return None;
                        }
                        Some(RuntimeCommand::CancelMission {
                            id: self
                                .model
                                .mission_id
                                .clone()
                                .unwrap_or_else(|| "current".to_string()),
                            reason: Some("Cancelled from composer (no runtime bridge)".to_string()),
                        })
                    }
                    ApplicationAction::ExitRequested => {
                        self.is_running = false;
                        self.send_or_fail("exit", ApplicationAction::ExitRequested);
                        None
                    }
                    // Canonical slash-command routing:
                    //   composer → parser → SlashCommandSubmitted →
                    //   runtime/registry authority → result → TUI.
                    // The TUI adds NAVIGATION side effects only
                    // (switching to the relevant surface); it never
                    // re-implements command execution. Registry-backed
                    // commands always travel through the bridge.
                    //
                    // Slash commands NEVER enter Thinking: they are not
                    // model reasoning. Only a silent request id is
                    // opened so the terminal CommandOutput/Error settles
                    // the request they belong to.
                    ApplicationAction::SlashCommandSubmitted {
                        ref command,
                        ref args,
                    } => {
                        self.model.begin_request_silent();
                        let cmd_lower = command.to_lowercase();
                        // Navigation side effects (presentation only).
                        match cmd_lower.as_str() {
                            "doctor" => {
                                self.navigation.navigate_to(ScreenId::Doctor);
                                self.model.settle_request();
                                return Some(RuntimeCommand::RunDoctor {
                                    category: None,
                                    json: false,
                                });
                            }
                            "settings" => {
                                if let Some(arg) = args.first() {
                                    if let Some(cat) =
                                        crate::tui::surface::settings::SettingsCategory::from_str_relaxed(arg)
                                    {
                                        self.settings_state.select_category(cat);
                                    }
                                }
                                self.navigation.navigate_to(ScreenId::Settings);
                                self.navigation
                                    .navigate_to_view(crate::tui::registry::ViewId::SettingsConfig);
                                self.normalize_focus();
                                self.unfocus_composer();
                                self.model.settle_request();
                                return None;
                            }
                            "agents" => {
                                self.navigation.navigate_to(ScreenId::Agents);
                                // View-only: no registry entry, no
                                // bridge execution to duplicate.
                                self.model.settle_request();
                                return None;
                            }
                            "tasks" => {
                                self.navigation.navigate_to(ScreenId::TaskGraph);
                            }
                            "tools" => {
                                self.navigation.navigate_to(ScreenId::Tools);
                            }
                            _ => {}
                        }
                        // Registry-backed execution path (single).
                        // `help` included: canonical help comes from
                        // the SlashCommandRegistry via the bridge.
                        if cmd_lower == "help" && self.bridge_tx.is_none() {
                            // Degraded fallback derives from the same
                            // registry authority — never a hardcoded
                            // command list that can drift.
                            let reg =
                                crate::interaction::commands::SlashCommandRegistry::new_standard();
                            self.model.settle_request();
                            self.model
                                .add_conversation_item(TuiConversationItem::System {
                                    text: reg.generate_help(None),
                                    timestamp: chrono::Utc::now(),
                                });
                            return None;
                        }
                        // View-only navigations settle immediately: no
                        // command result will arrive for them.
                        if self.bridge_tx.is_none()
                            && (cmd_lower == "tasks" || cmd_lower == "tools")
                        {
                            self.model.settle_request();
                            return None;
                        }
                        // `doctor`/`agents` already returned above.
                        // Unknown-to-registry names still travel the
                        // bridge so the runtime can answer with a
                        // readable "Unknown command" error card.
                        let label = format!("/{cmd_lower}");
                        self.send_or_fail(&label, routed);
                        None
                    }
                    ApplicationAction::UserTextSubmitted(mut parsed) => {
                        // Natural language: the ONLY path that owns
                        // model-thinking UI, entered after classification
                        // proves the input is not a slash command.
                        let request_id = uuid::Uuid::now_v7().to_string();
                        parsed.request_id = Some(request_id.clone());
                        self.model.active_request_id = Some(request_id);
                        self.model.active_command = None;
                        self.model.activity_kind = ActivityKind::Thinking;
                        self.model.activity_message = Some("Working on your request…".to_string());
                        self.model.activity_started_at = Some(chrono::Utc::now());
                        self.model.live_activity = self.model.activity_message.clone();
                        self.model.spinner.reset();
                        self.model.mark_dirty();
                        if self.bridge_tx.is_some() {
                            self.send_or_fail(
                                "prompt",
                                ApplicationAction::UserTextSubmitted(parsed),
                            );
                            return None;
                        }
                        // Fail closed: without the governed runtime
                        // bridge the TUI cannot enter the lifecycle,
                        // so it must not execute. A direct
                        // `RunMission` fallback would bypass plan
                        // review, task review, and authorization.
                        // The optimistic Thinking state above is
                        // revoked: the error card is the outcome.
                        let text = parsed.raw_text.clone();
                        self.model.fail_request("Bridge unavailable");
                        self.model.add_conversation_item(
                                TuiConversationItem::Error {
                                    message: format!(
                                        "Cannot submit {text:?}: governed runtime bridge unavailable; execution blocked."
                                    ),
                                    timestamp: chrono::Utc::now(),
                                },
                            );
                        self.model.add_log(
                            "ERROR",
                            "UserTextSubmitted dropped: no runtime bridge (fail-closed)",
                            "tui",
                        );
                        None
                    }
                    other => {
                        self.send_or_fail("request", other);
                        None
                    }
                }
            }
            ComposerAction::Cancel => {
                self.unfocus_composer();
                None
            }
            ComposerAction::None => None,
        }
    }

    /// Active workflow run id for dashboard-originated actions, if any.
    fn workflow_run_id(&self) -> Option<String> {
        self.workflow_snapshot
            .as_ref()
            .map(|s| s.run.id.to_string())
    }

    /// Unfocused mode owns the key: focus cycling, activation shortcuts,
    /// detail-panel keys, viewport scroll, and standard navigation routing.
    fn handle_route_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        // Settings Screen key handling when current screen is Settings
        if self.navigation.current_screen == ScreenId::Settings {
            if (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
                && !self.settings_state.editing
            {
                self.navigation.navigate_to(ScreenId::Dashboard);
                self.focus
                    .set_focus(crate::tui::focus::FocusTarget::Composer);
                self.is_composer_focused = true;
                self.model.mark_dirty();
                return None;
            }
            let rows = if let Some(ref cfg) = self.resolved_config {
                let mut effective_cfg = (**cfg).clone();
                if let Some(ref draft) = self.settings_draft {
                    effective_cfg.app_config = draft.clone();
                }
                let cat = self.settings_state.category();
                crate::tui::surface::settings::rows_for_category(
                    cat,
                    &effective_cfg,
                    self.model.catalog_snapshot.as_ref(),
                    &self.model.environment_overrides,
                    self.model.credential_status.is_configured(),
                )
            } else {
                Vec::new()
            };
            let row_count = rows.len();
            let editing_row = rows.get(self.settings_state.row_index);
            if let Some(action) = crate::tui::surface::settings::handle_settings_key(
                key,
                &mut self.settings_state,
                row_count,
                editing_row,
            ) {
                match action {
                    crate::tui::surface::settings::SettingsAction::Close => {
                        self.navigation.navigate_to(ScreenId::Dashboard);
                        self.focus
                            .set_focus(crate::tui::focus::FocusTarget::Composer);
                        self.is_composer_focused = true;
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::EditStarted => {
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::EditCommitted { key, value } => {
                        if let Some(ref mut draft) = self.settings_draft {
                            match crate::tui::surface::settings::apply_edit_to_draft(
                                draft, &key, &value,
                            ) {
                                Ok(restart_required) => {
                                    self.settings_state.message = if restart_required {
                                        Some(format!(
                                            "Updated {key} (restart required to take effect)"
                                        ))
                                    } else {
                                        Some(format!("Updated {key}"))
                                    };
                                }
                                Err(err) => {
                                    self.settings_state.message = Some(format!("Error: {err}"));
                                }
                            }
                        }
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::EditCancelled => {
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::SaveRequested => {
                        if let (Some(draft), Some(cfg)) =
                            (&self.settings_draft, &self.resolved_config)
                        {
                            let ws_root = &cfg.workspace_root;
                            match crate::tui::surface::settings::persist_workspace_config(
                                ws_root, draft,
                            ) {
                                Ok(path) => {
                                    match crate::config::ResolvedConfiguration::for_workspace(
                                        ws_root,
                                    ) {
                                        Ok(reloaded) => {
                                            let mut overrides_noted = Vec::new();
                                            if draft.agents.default_model != reloaded.active_model {
                                                if let Some(res) = reloaded
                                                    .provenance
                                                    .resolve("agents.default_model")
                                                {
                                                    if res.layer > crate::config::provenance::ConfigLayer::Tier3Workspace {
                                                        overrides_noted.push(format!(
                                                            "agents.default_model (overridden by {})",
                                                            res.layer.display_name()
                                                        ));
                                                    }
                                                }
                                            }
                                            if draft.provider.default != reloaded.active_provider {
                                                if let Some(res) =
                                                    reloaded.provenance.resolve("provider.default")
                                                {
                                                    if res.layer > crate::config::provenance::ConfigLayer::Tier3Workspace {
                                                        overrides_noted.push(format!(
                                                            "provider.default (overridden by {})",
                                                            res.layer.display_name()
                                                        ));
                                                    }
                                                }
                                            }

                                            let msg = if overrides_noted.is_empty() {
                                                format!("Saved to {}", path.display())
                                            } else {
                                                format!(
                                                    "Saved to {}, but active values overridden: {}",
                                                    path.display(),
                                                    overrides_noted.join(", ")
                                                )
                                            };
                                            self.settings_state.message = Some(msg);
                                            self.model.add_log(
                                                "INFO",
                                                format!(
                                                    "Settings persisted and reloaded from {}",
                                                    path.display()
                                                ),
                                                "settings",
                                            );

                                            let reloaded_arc = std::sync::Arc::new(reloaded);
                                            self.model.active_model =
                                                reloaded_arc.active_model.clone();
                                            self.model.active_provider =
                                                reloaded_arc.active_provider.clone();
                                            if let Some(ref prof) = reloaded_arc.active_profile {
                                                self.model.active_profile = prof.clone();
                                            }
                                            self.resolved_config = Some(reloaded_arc.clone());
                                            let channel =
                                                crate::deployment::DeploymentChannel::current();
                                            let creds = crate::runtime_authorities::resolve_runtime_credentials(
                                                &reloaded_arc.workspace_root,
                                                channel,
                                            );
                                            self.model.credential_status = if creds
                                                .api_key
                                                .is_some()
                                            {
                                                crate::tui::model::TuiCredentialStatus::Configured
                                            } else {
                                                crate::tui::model::TuiCredentialStatus::NotConfigured
                                            };
                                            if self.bridge_tx.is_some() {
                                                self.send_or_fail(
                                                    "configuration reconfigure",
                                                    ApplicationAction::ConfigurationReconfigureRequested {
                                                        config: reloaded_arc,
                                                    },
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            self.settings_state.message = Some(format!(
                                                "Saved to {}, but reload failed: {err}",
                                                path.display()
                                            ));
                                            self.model.add_log(
                                                "WARN",
                                                format!("Settings saved but reload failed: {err}"),
                                                "settings",
                                            );
                                        }
                                    }
                                }
                                Err(err) => {
                                    self.settings_state.message =
                                        Some(format!("Save failed: {err}"));
                                }
                            }
                        }
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::ResetToDefault { key } => {
                        let default_cfg = crate::config::AppConfig::default();
                        if let Some(ref mut draft) = self.settings_draft {
                            match crate::tui::surface::settings::reset_draft_field_to_default(
                                draft,
                                &default_cfg,
                                &key,
                            ) {
                                Ok(()) => {
                                    self.settings_state.message =
                                        Some(format!("Reset {key} to default value"));
                                }
                                Err(err) => {
                                    self.settings_state.message =
                                        Some(format!("Reset failed: {err}"));
                                }
                            }
                        }
                        self.model.mark_dirty();
                        return None;
                    }
                    crate::tui::surface::settings::SettingsAction::Noop => {
                        self.model.mark_dirty();
                        return None;
                    }
                }
            }
        }

        // If an interactive detail view is active, it owns keyboard navigation and actions.
        if let Some(detail) = self.navigation.active_detail {
            // Specialized DagInspector workflow key handling
            if detail == ViewId::DagInspector && self.workflow_snapshot.is_some() {
                if let Some(action) = handle_workflow_dashboard_key(
                    key,
                    &mut self.workflow_dashboard_state,
                    self.workflow_snapshot.as_ref().unwrap(),
                ) {
                    match action {
                        WorkflowAction::Resume => {
                            if let Some(run_id) = self.workflow_run_id() {
                                self.send_or_fail(
                                    "workflow resume",
                                    ApplicationAction::WorkflowResumeRequested { run_id },
                                );
                            }
                        }
                        WorkflowAction::Pause => {
                            if let Some(run_id) = self.workflow_run_id() {
                                self.send_or_fail(
                                    "workflow pause",
                                    ApplicationAction::WorkflowPauseRequested {
                                        run_id,
                                        reason: "Paused from workflow dashboard".to_string(),
                                    },
                                );
                            }
                        }
                        WorkflowAction::Cancel => {
                            if let Some(run_id) = self.workflow_run_id() {
                                self.send_or_fail(
                                    "workflow cancel",
                                    ApplicationAction::WorkflowCancelRequested {
                                        run_id,
                                        reason: "Cancelled from workflow dashboard".to_string(),
                                    },
                                );
                            }
                        }
                        WorkflowAction::Approve(step_key) => {
                            if let Some(run_id) = self.workflow_run_id() {
                                self.send_or_fail(
                                    "workflow approve",
                                    ApplicationAction::WorkflowApprovalSubmitted {
                                        run_id,
                                        step_key,
                                        approved: true,
                                        reason: None,
                                    },
                                );
                            }
                        }
                        WorkflowAction::Close => {
                            self.navigation.close_detail();
                            self.model.detail_scroll_offset = 0;
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
            }

            // Specialized ModelRegistry key handling
            if detail == ViewId::ModelRegistry {
                let (providers, models) =
                    crate::tui::surface::model_selector::resolve_display_models_and_providers(
                        &self.model.catalog_models,
                        &self.model.active_provider,
                        &self.model.active_model,
                    );

                if let Some(action) = handle_model_selector_key(
                    key,
                    &mut self.model_selector_state,
                    &providers,
                    &models,
                ) {
                    match action {
                        ModelSelectorAction::SwitchProvider(provider_id) => {
                            self.send_or_fail(
                                "switch provider",
                                ApplicationAction::ProviderChangeRequested {
                                    provider: provider_id,
                                },
                            );
                        }
                        ModelSelectorAction::SetPrimaryModel(model_id) => {
                            self.send_or_fail(
                                "set primary model",
                                ApplicationAction::ModelChangeRequested { model: model_id },
                            );
                        }
                        ModelSelectorAction::SetFastModel(model_id) => {
                            self.send_or_fail(
                                "set fast model",
                                ApplicationAction::ModelChangeRequested { model: model_id },
                            );
                        }
                        ModelSelectorAction::Close => {
                            self.navigation.close_detail();
                            self.model.detail_scroll_offset = 0;
                            self.model.mark_dirty();
                        }
                    }
                    return None;
                }
            }

            // General detail inspector key handling (scrolling, Esc/q close, Quarantine retry)
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.navigation.close_detail();
                    self.model.detail_scroll_offset = 0;
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.model.detail_scroll_offset =
                        self.model.detail_scroll_offset.saturating_sub(1);
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.model.detail_scroll_offset =
                        self.model.detail_scroll_offset.saturating_add(1);
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::PageUp => {
                    self.model.detail_scroll_offset =
                        self.model.detail_scroll_offset.saturating_sub(10);
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::PageDown => {
                    self.model.detail_scroll_offset =
                        self.model.detail_scroll_offset.saturating_add(10);
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::Home => {
                    self.model.detail_scroll_offset = 0;
                    self.model.mark_dirty();
                    return None;
                }
                KeyCode::Char('r') | KeyCode::Char('R') if detail == ViewId::QuarantineManager => {
                    let gen_id = self.model.quarantine_generation.saturating_add(1);
                    self.model.quarantine_generation = gen_id;
                    self.model.quarantine_state =
                        crate::tui::model::QuarantineProjectionState::Loading;
                    if !self.send_or_fail(
                        "refresh quarantine",
                        ApplicationAction::QuarantineRefreshRequested { generation: gen_id },
                    ) {
                        self.model.set_quarantine_failed(
                            "Quarantine query was not dispatched; retry when the runtime bridge is available.",
                        );
                    }
                    self.model.mark_dirty();
                    return None;
                }
                _ => {
                    return None;
                }
            }
        }

        // When composer is unfocused: focus cycling, activation shortcuts, scrolling, and navigation
        if key.code == KeyCode::Tab {
            let has_rail = self.has_context_panel();
            self.focus.cycle_next(has_rail);
            if self.focus.current() == FocusTarget::Composer {
                self.focus_composer();
            } else {
                self.is_composer_focused = false;
            }
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::BackTab {
            let has_rail = self.has_context_panel();
            self.focus.cycle_prev(has_rail);
            if self.focus.current() == FocusTarget::Composer {
                self.focus_composer();
            } else {
                self.is_composer_focused = false;
            }
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::Char('i') {
            self.focus_composer();
            return None;
        }

        if key.code == KeyCode::Enter {
            if self.model.has_inspectable_error() {
                self.navigation.open_detail(ViewId::FailureRecovery);
                self.model.mark_dirty();
                return None;
            }
            self.focus_composer();
            return None;
        }

        if key.code == KeyCode::Char('/') {
            self.focus_composer();
            self.composer.set_text("/");
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::Char('@') {
            self.focus_composer();
            self.composer.set_text("@");
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::Char('?') {
            self.overlay_manager.toggle_help();
            if self.overlay_manager.is_help_open {
                self.focus.enter_overlay();
            } else {
                self.focus.exit_overlay();
            }
            self.model.mark_dirty();
            return None;
        }

        if self.navigation.current_screen == ScreenId::Jobs {
            let total_jobs = self.model.jobs.len();
            if total_jobs > 0 {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.model.jobs_selected_idx =
                            (self.model.jobs_selected_idx + 1).min(total_jobs.saturating_sub(1));
                        self.model.mark_dirty();
                        return None;
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.model.jobs_selected_idx =
                            self.model.jobs_selected_idx.saturating_sub(1);
                        self.model.mark_dirty();
                        return None;
                    }
                    KeyCode::PageDown => {
                        self.model.jobs_selected_idx =
                            (self.model.jobs_selected_idx + 10).min(total_jobs.saturating_sub(1));
                        self.model.mark_dirty();
                        return None;
                    }
                    KeyCode::PageUp => {
                        self.model.jobs_selected_idx =
                            self.model.jobs_selected_idx.saturating_sub(10);
                        self.model.mark_dirty();
                        return None;
                    }
                    KeyCode::Home => {
                        self.model.jobs_selected_idx = 0;
                        self.model.mark_dirty();
                        return None;
                    }
                    KeyCode::End => {
                        self.model.jobs_selected_idx = total_jobs.saturating_sub(1);
                        self.model.mark_dirty();
                        return None;
                    }
                    _ => {}
                }
            }
        }

        if key.code == KeyCode::PageUp {
            let step = self.model.page_step();
            self.model.scroll_up(step);
            return None;
        }

        if key.code == KeyCode::PageDown {
            let step = self.model.page_step();
            self.model.scroll_down(step);
            return None;
        }

        if key.code == KeyCode::End {
            self.model.scroll_to_bottom();
            return None;
        }

        if key.code == KeyCode::Home {
            self.model.scroll_to_top();
            return None;
        }

        // Contextual j/k navigation when context panel is focused
        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
            if self.focus.current() == FocusTarget::ContextPanel {
                self.context_selected_idx = self.context_selected_idx.saturating_add(1);
                self.diff_scroll_offset = self.diff_scroll_offset.saturating_add(1);
                self.normalize_context_rail_state();
                self.model.mark_dirty();
                return None;
            }
            if self.focus.current() == FocusTarget::Conversation {
                self.model.scroll_down(1);
                return None;
            }
        }

        if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
            if self.focus.current() == FocusTarget::ContextPanel {
                self.context_selected_idx = self.context_selected_idx.saturating_sub(1);
                self.diff_scroll_offset = self.diff_scroll_offset.saturating_sub(1);
                self.normalize_context_rail_state();
                self.model.mark_dirty();
                return None;
            }
            if self.focus.current() == FocusTarget::Conversation {
                self.model.scroll_up(1);
                return None;
            }
        }

        // 7. Standard navigation routing (1-0, l, m, a, r, Space, Esc, q)
        match self.navigation.handle_key(key) {
            NavigationAction::ScreenChanged(screen) => {
                if screen == ScreenId::Settings {
                    self.focus.enter_settings();
                    self.is_composer_focused = false;
                } else if self.focus.is_settings() {
                    self.focus.exit_settings();
                }
                self.normalize_context_rail_state();
                self.model.mark_dirty();
                None
            }
            NavigationAction::OpenPalette => {
                self.palette.open();
                self.model.mark_dirty();
                None
            }
            NavigationAction::TogglePause => {
                self.model.mark_dirty();
                // §47: pause/resume requires a real mission id. The legacy
                // "current" placeholder never resolves and failed honestly
                // nowhere — now it fails honestly HERE with guidance.
                let Some(ref mission_id) = self.model.mission_id else {
                    self.model.add_conversation_item(
                        crate::tui::conversation::TuiConversationItem::Error {
                            message: "No active mission to pause/resume.".to_string(),
                            timestamp: chrono::Utc::now(),
                        },
                    );
                    return None;
                };
                if self.bridge_tx.is_some() {
                    if self.model.mission_status == "paused" {
                        self.send_or_fail(
                            "resume mission",
                            ApplicationAction::MissionResumeRequested {
                                mission_id: mission_id.clone(),
                            },
                        );
                    } else {
                        self.send_or_fail(
                            "pause mission",
                            ApplicationAction::MissionPauseRequested {
                                mission_id: mission_id.clone(),
                            },
                        );
                    }
                    return None;
                }
                if self.model.mission_status == "paused" {
                    Some(RuntimeCommand::ResumeMission {
                        id: mission_id.clone(),
                    })
                } else {
                    Some(RuntimeCommand::PauseMission {
                        id: mission_id.clone(),
                    })
                }
            }
            NavigationAction::Quit => {
                self.is_running = false;
                None
            }
            NavigationAction::None => None,
        }
    }

    /// Render a single frame with dirty-state scheduling and zero DB I/O.
    pub fn render_frame<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> std::io::Result<bool> {
        if let Ok(sz) = terminal.size() {
            if sz.width != self.last_area.width || sz.height != self.last_area.height {
                self.last_area = Rect::new(0, 0, sz.width, sz.height);
                self.force_redraw = true;
                self.normalize_focus();
                self.normalize_context_rail_state();
            }
        }

        let is_dirty = if self.replay.is_active {
            self.replay_model
                .as_ref()
                .map(|m| m.is_dirty)
                .unwrap_or(false)
        } else {
            self.model.is_dirty
        };
        if !is_dirty && !self.force_redraw && !self.model.has_active_animation() {
            return Ok(false);
        }

        // Snapshot sqlite query counter before rendering
        let pre_render_queries = self.model.sqlite_render_access_count();

        terminal
            .draw(|f| {
                let area = f.area();
                let (_tier, layout_areas) = compute_layout(area);
                let tokens = ThemeTokens::resolve(self.theme_mode);

                let display_model = if self.replay.is_active {
                    self.replay_model.as_ref().unwrap_or(&self.model)
                } else {
                    &self.model
                };

                // 1. Persistent Shell Header
                render_header(
                    f,
                    layout_areas.header,
                    display_model,
                    self.navigation.current_screen,
                    &self.replay,
                    &tokens,
                );

                // 2. Unified Workspace (Conversation + Contextual Inspector + Docked Composer)
                let header_h = layout_areas.header.height.min(area.height);
                let footer_h = layout_areas
                    .footer
                    .height
                    .min(area.height.saturating_sub(header_h));
                let ws_height = area.height.saturating_sub(header_h + footer_h);
                let workspace_area = Rect {
                    x: area.x,
                    y: area.y + header_h,
                    width: area.width,
                    height: ws_height,
                };
                let active_model = if self.replay.is_active {
                    self.replay_model.as_mut().unwrap_or(&mut self.model)
                } else {
                    &mut self.model
                };
                render_workspace(
                    f,
                    workspace_area,
                    active_model,
                    &self.composer,
                    self.navigation.current_screen,
                    &self.replay,
                    self.focus.current(),
                    self.is_composer_focused,
                    &tokens,
                    self.context_selected_idx,
                    self.navigation.active_detail,
                    self.navigation.active_overlay,
                    &self.workflow_snapshot,
                    &mut self.workflow_dashboard_state,
                    &mut self.model_selector_state,
                    &mut self.setup_wizard,
                    &mut self.settings_state,
                    self.resolved_config.as_deref(),
                    self.settings_draft.as_ref(),
                );

                let footer_model = if self.replay.is_active {
                    self.replay_model.as_ref().unwrap_or(&self.model)
                } else {
                    &self.model
                };
                // 3. Context-Sensitive Shell Footer
                super::shell::footer::render_footer_full(
                    f,
                    layout_areas.footer,
                    self.navigation.current_screen,
                    self.focus.current(),
                    self.is_composer_focused,
                    &footer_model.lifecycle,
                    footer_model.follow,
                    footer_model.unseen_count,
                    &tokens,
                );

                // 4. Render Floating Command Palette if open
                if self.palette.is_open {
                    self.palette.render_with_model(f, area, Some(&self.model));
                }

                // 5. Render Floating Approval Modal if open
                if self.approval_modal.is_open {
                    self.approval_modal.render_with_theme(
                        f,
                        area,
                        &ThemeTokens::resolve(self.theme_mode),
                    );
                }

                // 6. Render Floating Help Overlay if open
                if self.overlay_manager.is_help_open {
                    let tokens = ThemeTokens::resolve(self.theme_mode);
                    render_help_overlay(f, area, &tokens);
                }
            })
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        // Invariant check: zero sqlite queries during render
        let post_render_queries = self.model.sqlite_render_access_count();
        assert_eq!(
            pre_render_queries, post_render_queries,
            "INVARIANT VIOLATION: SQLite queries executed during terminal.draw() (TUI-03, D-15)"
        );

        self.model.spinner.tick();
        self.model.clear_dirty();
        if let Some(ref mut rm) = self.replay_model {
            rm.clear_dirty();
        }
        self.force_redraw = false;

        Ok(true)
    }

    /// Step one tick of the decoupled TUI loop.
    pub fn tick<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<bool> {
        self.poll_updates();
        if self.replay.is_active && self.replay.is_playing && self.replay.tick_playback() {
            self.replay_model = Some(self.replay.reconstruct_current_view());
            self.model.mark_dirty();
        }
        if let Some(ref mut wizard) = self.setup_wizard {
            if wizard.poll_discovery() {
                self.model.mark_dirty();
            }
        }
        self.render_frame(terminal)
    }
}

/// Classify a mouse event (scroll only; other gestures ignored).
pub fn classify_mouse(kind: MouseEventKind) -> Option<bool> {
    match kind {
        MouseEventKind::ScrollUp => Some(true),
        MouseEventKind::ScrollDown => Some(false),
        _ => None,
    }
}

/// Derive the parser prompt state from the canonical lifecycle projection.
///
/// §52: the composer must respect governed state. Review and authorization
/// stages route `y/n/cancel` as approval answers; discovery/execution stages
/// keep free-text semantics. This is UI-local input routing only — the
/// runtime coordinator remains the governance authority.
pub fn composer_prompt_state(
    stage: &crate::tui::lifecycle::TuiLifecycleStage,
) -> crate::interaction::state::SessionPromptState {
    use crate::interaction::state::SessionPromptState as S;
    use crate::tui::lifecycle::TuiLifecycleStage as L;
    match stage {
        L::PlanReviewRequired
        | L::PlanRevisionAvailable
        | L::TasksReviewRequired
        | L::TaskRevisionAvailable
        | L::ExecutionAuthorizationRequired
        | L::PlanAccepted
        | L::TasksAccepted
        | L::ExecutionAuthorized => S::AwaitingApproval,
        L::DiscoveryRequired | L::Blocked => S::WaitingForUser,
        L::Executing | L::Verifying => S::Executing,
        L::Completed | L::Failed | L::Rejected | L::Cancelled => S::Completing,
        L::Idle | L::IntentActive | L::PlanDraft | L::TasksDraft => S::Idle,
    }
}
