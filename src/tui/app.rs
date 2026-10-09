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

    /// Scroll the authoritative conversation viewport from a mouse wheel.
    /// Wheel-up moves toward older content; wheel-down toward newer.
    /// No-op while a modal overlay owns input.
    pub fn handle_mouse_scroll(&mut self, up: bool) {
        if self.palette.is_open || self.approval_modal.is_open || self.overlay_manager.is_help_open
        {
            return;
        }
        self.model.scroll_wheel(up);
    }

    pub fn with_workspace_root(mut self, root: PathBuf) -> Self {
        self.model.workspace_path = root.display().to_string();
        self.composer = TuiComposer::new(root);
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
        if let Ok(records) = runtime.artifact_service().list_all().await {
            let snaps = records
                .iter()
                .map(crate::tui::model::TuiArtifactSnapshot::from)
                .collect();
            self.model.load_artifacts(snaps);
        }

        // 2. Verification checks from EvidenceCompletionGate
        if let Ok(checks) = runtime.completion_gate().list_all_checks().await {
            let snaps = checks
                .iter()
                .map(crate::tui::model::TuiVerificationCheck::from)
                .collect();
            self.model.load_verification_checks(snaps);
        }

        // 3. Recovery attempts from RecoveryBudgetTracker
        if let Ok(attempts) =
            crate::recovery::budget::RecoveryBudgetTracker::query_all_attempts(runtime.pool()).await
        {
            let snaps = attempts
                .iter()
                .map(crate::tui::model::TuiRecoverySnapshot::from)
                .collect();
            self.model.load_recovery_attempts(snaps);
        }

        // 4. Atomic budget snapshot from BudgetEnforcer
        let snap = runtime.budget_enforcer().snapshot();
        self.model.update_budget((&snap).into());

        // 5. Canonical tools from ToolRegistry
        let tools = runtime.tool_registry().list_tools();
        let snaps = tools
            .iter()
            .map(|t| crate::tui::model::TuiToolDefinitionSnapshot::from(t.as_ref()))
            .collect();
        self.model.load_canonical_tools(snaps);

        // 6. Discovered skills
        let root = std::path::Path::new(&self.model.workspace_path);
        let skills = crate::skill::discovery::SkillDiscovery::discover_all(Some(root));
        let snaps = skills
            .iter()
            .map(crate::tui::model::TuiSkillSnapshot::from)
            .collect();
        self.model.load_skills(snaps);

        // 7. Authoritative Git state via GitService
        if let Ok(status) = runtime.git_service().status().await {
            self.model.git_branch = if status.branch.is_empty() {
                "N/A".to_string()
            } else {
                status.branch
            };
        }

        // 8. Authoritative active task graph if one exists for the active mission
        use crate::persistence::sqlite::repositories::TaskGraphRepository;
        let graph_repo = crate::persistence::sqlite::repositories::SqliteTaskGraphRepository::new(
            runtime.pool().clone(),
        );
        let graph_opt = if let Some(ref mid_str) = self.model.mission_id {
            if let Ok(mid) = mid_str.parse::<crate::ids::MissionId>() {
                graph_repo.get_active_graph(mid).await.unwrap_or(None)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(graph) = graph_opt {
            let snaps = graph
                .task_summaries()
                .iter()
                .map(|t| crate::tui::model::TuiTaskSnapshot {
                    id: t.id.to_string(),
                    title: t.title.clone(),
                    status: t.status.to_string(),
                    agent_role: Some(t.role.to_string()),
                    progress_pct: if t.status == crate::state_machine::task::TaskState::Succeeded {
                        100
                    } else {
                        0
                    },
                    dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
                    ..Default::default()
                })
                .collect();
            self.model.load_tasks(snaps);
        }

        // 9. Authoritative model usage telemetry
        let inv_repo = crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
            runtime.pool().clone(),
        );
        if let Ok(records) = inv_repo.get_all_invocations().await {
            for r in records {
                let inv_id = r.id.to_string();
                if self.model.model_usage.processed_invocations.insert(inv_id) {
                    self.model.model_usage.prompt_tokens += r.prompt_tokens as u64;
                    self.model.model_usage.completion_tokens += r.completion_tokens as u64;
                    self.model.model_usage.api_calls += 1;
                }
            }
        }

        // 10. Load canonical model catalog authority (global cache,
        // legacy workspace cache as migration fallback) — same source the
        // runtime refresh persists to, so CLI/TUI/wizard observe one catalog.
        let ws = std::path::Path::new(&self.model.workspace_path);
        if let Some(catalog) = crate::model::catalog::ModelCatalog::load_canonical_for_workspace(ws)
        {
            self.model.catalog_models = catalog.models;
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
        if self.is_composer_focused {
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

    /// Reconstruct complete view state from historical events upon reconnect (TUI-05).
    pub fn reconnect(&mut self, history: &[EventEnvelope]) {
        self.model.reconstruct_from_events(history);
        self.force_redraw = true;
    }

    /// Enter replay mode with specified historical events.
    pub fn enter_replay(&mut self, history: Vec<EventEnvelope>) {
        self.replay.load_history(history);
        self.model = self.replay.reconstruct_current_view();
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
        self.assembly_rx = Some(rx);
        self.assembly_settled = false;
        self.assembly_started_at = Some(std::time::Instant::now());
        self.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    }

    /// Attach the async runtime-assembly receiver along with an explicit task abort handle.
    /// When assembly times out or fails, the abort handle cancels the background task.
    pub fn begin_assembly_with_abort_handle(
        &mut self,
        rx: UnboundedReceiver<RuntimeAssemblyOutcome>,
        abort_handle: tokio::task::AbortHandle,
    ) {
        self.assembly_rx = Some(rx);
        self.assembly_settled = false;
        self.assembly_started_at = Some(std::time::Instant::now());
        self.assembly_abort_handle = Some(abort_handle);
        self.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    }

    /// Poll async runtime assembly without blocking render.
    ///
    /// On `Ready`: walks the phased hydration
    /// (`HydratingSession → HydratingWorkspace → HydratingExecution → Ready`),
    /// drawing each step before its work so partial data stays visible.
    /// On failure or deadline exceeded: records a visible typed error (never blank).
    pub async fn poll_runtime<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> bool {
        if self.assembly_settled {
            return false;
        }

        // Bounded startup deadline: a stalled runtime assembly must transition
        // to visible Failed state rather than hanging indefinitely.
        if let Some(started_at) = self.assembly_started_at {
            if started_at.elapsed() >= self.assembly_timeout {
                self.assembly_settled = true;
                if let Some(ref handle) = self.assembly_abort_handle {
                    handle.abort();
                }
                let err = TuiError::runtime(format!(
                    "Runtime assembly timed out after {:.1}s (deadline exceeded)",
                    self.assembly_timeout.as_secs_f64()
                ));
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
        match outcome {
            RuntimeAssemblyOutcome::Ready(rt) => {
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
            self.resolved_config = Some(rt.config().clone());
            self.settings_draft = Some(rt.config().app_config.clone());
        }
        // Announce workspace hydration:
        self.set_runtime_hydrating_workspace(Some("Loading workspace state…".to_string()));
        let _ = self.render_frame(terminal);
        let hydrate_res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.hydrate_from_runtime(&rt),
        )
        .await;
        if hydrate_res.is_err() {
            tracing::warn!(
                "Workspace state hydration timed out after 10s; continuing in degraded mode"
            );
        }

        // Announce bridge connect next:
        self.set_runtime_hydrating_execution(Some("Connecting cockpit bridge…".to_string()));
        let _ = self.render_frame(terminal);

        let attach_res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.binding.attach_runtime(rt.clone(), None),
        )
        .await;

        match attach_res {
            Ok(Ok(())) => {
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
                self.binding.record_failure(err.clone());
                self.set_runtime_failed_with_kind(err.kind, err.message.clone());
                let _ = self.render_frame(terminal);
            }
            Err(_) => {
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

    /// Classify the next key under the single input-priority authority
    /// (`keymap::resolve_key`): replay → dialog → palette → help → wizard →
    /// palette-chord → interrupt → composer → route.
    ///
    /// The classification is CONSUMED by `handle_key`, never discarded.
    pub fn classify_key(&mut self, key: KeyEvent) -> KeyAction {
        let ctx = KeyContext::new(
            self.replay.is_active,
            self.approval_modal.is_open,
            self.palette.is_open,
            self.overlay_manager.is_help_open,
            self.navigation.active_overlay == Some(ViewId::SetupWizard),
            self.is_composer_focused,
        );
        resolve_key(key, ctx)
    }

    /// Exit chord check (q / Ctrl+C / Ctrl+D).
    pub fn should_exit(&self, key: KeyEvent) -> bool {
        let no_dialog = !self.approval_modal.is_open
            && !self.palette.is_open
            && !self.overlay_manager.is_help_open;
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
    /// interrupt → composer → route); each owner below handles keys and
    /// translates them to `ApplicationAction`s. No second matcher exists.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        match self.classify_key(key) {
            KeyAction::Replay => {
                // Read-only scrub: mutation commands are blocked (D-19, T-11-20).
                if self.replay.handle_key(key) {
                    self.model = self.replay.reconstruct_current_view();
                    self.model.mark_dirty();
                }
                None
            }
            KeyAction::Dialog => self.handle_dialog_key(key),
            KeyAction::Palette => self.handle_palette_key(key),
            KeyAction::Help => self.handle_help_key(key),
            KeyAction::Wizard => self.handle_wizard_key(key),
            KeyAction::OpenPalette => {
                self.palette.open();
                self.focus.enter_overlay();
                self.model.mark_dirty();
                None
            }
            KeyAction::Exit => self.handle_cancel_key(key),
            KeyAction::Composer => self.handle_composer_key(key),
            KeyAction::Route | KeyAction::Global => self.handle_route_key(key),
        }
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
        let request_id = self
            .approval_modal
            .current_request
            .as_ref()
            .map(|r| r.id.clone());
        if let Some(decision) = self.approval_modal.handle_key(key) {
            self.model.mark_dirty();
            self.focus.exit_overlay();
            if let Some(approval_id) = request_id {
                // Session scope (AllowForSession) is deliberate here: the TUI
                // governs an interactive session, while headless CLI
                // ResolveApproval remains mission-scoped.
                self.send_or_fail(
                    "resolve approval",
                    ApplicationAction::ApprovalDecision {
                        request_id: approval_id.clone(),
                        decision,
                    },
                );
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
                    // Single canonical navigation authority: every
                    // registered view resolves to a functional route or a
                    // contextual detail/overlay on its parent route.
                    self.navigation.navigate_to_view(view_id);
                    self.model.mark_dirty();
                }
                PaletteActionV2::ExecuteCommand(cmd) => {
                    self.model.enter_active_session();
                    self.model.mark_dirty();
                    if self.bridge_tx.is_some() {
                        match cmd {
                            RuntimeCommand::PauseMission { id } => {
                                let mission_id = if id == "current" {
                                    self.model.mission_id.clone().unwrap_or_default()
                                } else {
                                    id
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
                                    id
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
                            _ => return Some(cmd),
                        }
                        return None;
                    }
                    return Some(cmd);
                }
                PaletteActionV2::Action(act_str) => {
                    self.model.mark_dirty();
                    if act_str == "toggle_theme" {
                        self.theme_mode = self.theme_mode.cycle();
                    } else if let (Some(stripped), true) =
                        (act_str.strip_prefix('/'), self.bridge_tx.is_some())
                    {
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
        if let Some(ref mut wizard) = self.setup_wizard {
            let outcome = wizard.handle_key(key);
            self.model.mark_dirty();
            match outcome {
                WizardOutcome::Completed => {
                    if let Err(e) = wizard.persist_configuration() {
                        wizard.status_message = Some(format!("Failed to persist config: {e}"));
                    } else {
                        self.navigation.close_overlay();
                        self.focus.exit_overlay();
                        self.model.add_log(
                            "INFO",
                            "Setup Wizard completed and configuration persisted",
                            "wizard",
                        );
                    }
                }
                WizardOutcome::Cancelled => {
                    self.navigation.close_overlay();
                    self.focus.exit_overlay();
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
            self.focus.cycle_next(true);
            self.is_composer_focused = self.focus.current() == FocusTarget::Composer;
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::BackTab {
            self.focus.cycle_prev(true);
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
                        self.model.conversation.clear();
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
            if key.code == KeyCode::Char('q') && !self.settings_state.editing {
                self.navigation.navigate_to(ScreenId::Dashboard);
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
                    None,
                    &std::collections::HashMap::new(),
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

                                            self.model.active_model = reloaded.active_model.clone();
                                            self.model.active_provider =
                                                reloaded.active_provider.clone();
                                            if let Some(ref prof) = reloaded.active_profile {
                                                self.model.active_profile = prof.clone();
                                            }
                                            let active_model = reloaded.active_model.clone();
                                            let active_provider = reloaded.active_provider.clone();
                                            let active_profile = reloaded.active_profile.clone();
                                            self.resolved_config =
                                                Some(std::sync::Arc::new(reloaded));
                                            if self.bridge_tx.is_some() {
                                                self.send_or_fail(
                                                    "model change",
                                                    ApplicationAction::ModelChangeRequested {
                                                        model: active_model,
                                                    },
                                                );
                                                self.send_or_fail(
                                                    "provider change",
                                                    ApplicationAction::ProviderChangeRequested {
                                                        provider: active_provider,
                                                    },
                                                );
                                                if let Some(prof) = active_profile {
                                                    self.send_or_fail(
                                                        "profile change",
                                                        ApplicationAction::ProfileChangeRequested {
                                                            profile: prof,
                                                        },
                                                    );
                                                }
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
                    crate::tui::surface::settings::SettingsAction::ResetToDefault { .. } => {
                        if let Some(ref cfg) = self.resolved_config {
                            self.settings_draft = Some(cfg.app_config.clone());
                            self.settings_state.message =
                                Some("Draft reset from active config".to_string());
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

        // When composer is unfocused: focus cycling, activation shortcuts, scrolling, and navigation
        if key.code == KeyCode::Tab {
            self.focus.cycle_next(true);
            if self.focus.current() == FocusTarget::Composer {
                self.focus_composer();
            } else {
                self.is_composer_focused = false;
            }
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::BackTab {
            self.focus.cycle_prev(true);
            if self.focus.current() == FocusTarget::Composer {
                self.focus_composer();
            } else {
                self.is_composer_focused = false;
            }
            self.model.mark_dirty();
            return None;
        }

        if key.code == KeyCode::Char('i') || key.code == KeyCode::Enter {
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

        // 6. Workflow Dashboard key handling when workflow detail is active
        if let Some(detail) = self.navigation.active_detail
            && detail == ViewId::DagInspector
            && self.workflow_snapshot.is_some()
        {
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
                        self.model.mark_dirty();
                    }
                }
                return None;
            }
        }

        // 7. Model Selector key handling when ModelRegistry detail is active
        if let Some(detail) = self.navigation.active_detail
            && detail == ViewId::ModelRegistry
        {
            let (providers, models) =
                crate::tui::surface::model_selector::resolve_display_models_and_providers(
                    &self.model.catalog_models,
                    &self.model.active_provider,
                    &self.model.active_model,
                );

            if let Some(action) =
                handle_model_selector_key(key, &mut self.model_selector_state, &providers, &models)
            {
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
                        self.model.mark_dirty();
                    }
                }
                return None;
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
            NavigationAction::ScreenChanged(_) => {
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
        if !self.model.is_dirty && !self.force_redraw && !self.model.has_active_animation() {
            return Ok(false);
        }

        // Snapshot sqlite query counter before rendering
        let pre_render_queries = self.model.sqlite_render_access_count();

        terminal
            .draw(|f| {
                let area = f.area();
                let (_tier, layout_areas) = compute_layout(area);
                let tokens = ThemeTokens::resolve(self.theme_mode);

                // 1. Persistent Shell Header
                render_header(
                    f,
                    layout_areas.header,
                    &self.model,
                    self.navigation.current_screen,
                    &self.replay,
                    &tokens,
                );

                // 2. Unified Workspace (Conversation + Contextual Inspector + Docked Composer)
                let workspace_area = Rect {
                    x: area.x,
                    y: layout_areas.header.bottom(),
                    width: area.width,
                    height: area
                        .height
                        .saturating_sub(layout_areas.header.height + layout_areas.footer.height),
                };
                render_workspace(
                    f,
                    workspace_area,
                    &mut self.model,
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

                // 3. Context-Sensitive Shell Footer
                super::shell::footer::render_footer_full(
                    f,
                    layout_areas.footer,
                    self.navigation.current_screen,
                    self.focus.current(),
                    self.is_composer_focused,
                    &self.model.lifecycle,
                    self.model.follow,
                    self.model.unseen_count,
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
        self.force_redraw = false;

        Ok(true)
    }

    /// Step one tick of the decoupled TUI loop.
    pub fn tick<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<bool> {
        self.poll_updates();
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
