//! Decoupled TUI Application Engine & Render Loop (TUI-03, D-15, TUI 2.0).
//!
//! Enforces Law 9: "The TUI is a projection, never authoritative state."
//! Features dirty-state frame scheduling (30–60 FPS), sub-5ms render budget,
//! bounded update consumption, and zero database queries during draw.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use super::approval::ApprovalModal;
use super::channel::TuiUpdateReceiver;
use super::composer::{ComposerAction, TuiComposer};
use super::conversation::TuiConversationItem;
use super::focus::{FocusManager, FocusTarget};
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

/// TUI Cockpit Application State and Controller.
pub struct TuiApp {
    pub model: TuiViewModel,
    pub navigation: NavigationRouter,
    pub palette: UniversalCommandPalette,
    pub approval_modal: ApprovalModal,
    pub overlay_manager: OverlayManager,
    pub replay: ReplayController,
    pub focus: FocusManager,
    pub context_selected_idx: usize,
    pub diff_scroll_offset: usize,
    pub receiver: Option<TuiUpdateReceiver>,
    pub is_running: bool,
    pub frame_count: u64,
    pub last_render_duration_micros: u64,
    pub target_fps: u32,
    pub force_redraw: bool,
    pub composer: TuiComposer,
    pub is_composer_focused: bool,
    pub interaction_rx: Option<UnboundedReceiver<InteractionEvent>>,
    pub bridge_tx: Option<UnboundedSender<ApplicationAction>>,
    pub theme_mode: ThemeMode,
    /// Workflow dashboard interaction state
    pub workflow_dashboard_state: WorkflowDashboardState,
    /// Cached workflow snapshot for dashboard rendering
    pub workflow_snapshot: Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
    /// Setup Wizard state - accessible via View 22
    pub setup_wizard: Option<SetupWizardScreen>,
    /// Model selector state - accessible via View 24
    pub model_selector_state: ModelSelectorState,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
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
            receiver: None,
            is_running: true,
            frame_count: 0,
            last_render_duration_micros: 0,
            target_fps: 60,
            force_redraw: true,
            composer: TuiComposer::new(PathBuf::from(".")),
            is_composer_focused: true,
            interaction_rx: None,
            bridge_tx: None,
            // Pre-hydration placeholder: always overwritten by `with_config`
            // (resolved configuration) before production render. Uses the
            // canonical default so even an unhydrated app agrees with the
            // resolver on the effective theme.
            theme_mode: ThemeMode::canonical_default(),
            workflow_dashboard_state: WorkflowDashboardState::new(),
            workflow_snapshot: None,
            setup_wizard: None,
            model_selector_state: ModelSelectorState::new(),
        }
    }

    pub fn with_receiver(mut self, receiver: TuiUpdateReceiver) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_interaction_rx(mut self, rx: UnboundedReceiver<InteractionEvent>) -> Self {
        self.interaction_rx = Some(rx);
        self
    }

    pub fn with_bridge_tx(mut self, tx: UnboundedSender<ApplicationAction>) -> Self {
        self.bridge_tx = Some(tx);
        self
    }

    /// Send an action to the governed runtime bridge, failing EXPLICITLY.
    ///
    /// A dropped/failed channel send must never become a silent Thinking
    /// state: the request is revoked into a visible error instead.
    /// Returns true when the runtime accepted the action.
    fn send_or_fail(&mut self, op: &str, action: ApplicationAction) -> bool {
        let accepted = self
            .bridge_tx
            .as_ref()
            .map(|tx| tx.send(action).is_ok())
            .unwrap_or(false);
        if !accepted {
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
        }
        accepted
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
        self.target_fps = config.app_config.tui.fps;
        let mode = ThemeMode::from_str_relaxed(&config.app_config.tui.theme);
        self.theme_mode = mode;
        self.model.active_model = config.active_model.clone();
        self.model.active_provider = config.active_provider.clone();
        if let Some(ref prof) = config.active_profile {
            self.model.active_profile = prof.clone();
        }
        self
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

    /// Live reload user commands into TuiComposer and UniversalCommandPalette without recreating TuiApp.
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

    /// Mark the TUI as hydrating durable state.
    pub fn set_runtime_hydrating(&mut self, detail: Option<String>) {
        self.model.set_runtime_hydrating(detail);
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

    /// Process pending events from both kernel envelope and interaction event channels.
    pub fn poll_updates(&mut self) -> usize {
        let mut count = 0;
        if let Some(ref mut rx) = self.receiver {
            let events = rx.drain_available();
            count += events.len();
            for event in events {
                self.model.apply_event(&event);
            }
        }

        if let Some(ref mut irx) = self.interaction_rx {
            while let Ok(ie) = irx.try_recv() {
                if let InteractionEvent::WorkflowSnapshotUpdated { ref snapshot } = ie {
                    self.workflow_snapshot = Some((**snapshot).clone());
                }
                self.model.apply_interaction_event(&ie);
                count += 1;
            }
        }
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

    /// Handle keyboard input, returning any mutating `RuntimeCommand` to dispatch.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<RuntimeCommand> {
        // 1. Replay mode handles keys first; mutation commands are blocked (D-19, T-11-20)
        if self.replay.is_active {
            if self.replay.handle_key(key) {
                self.model = self.replay.reconstruct_current_view();
                self.model.mark_dirty();
            }
            return None;
        }

        // 2. Interactive Approval Modal handles keys if active (D-17)
        if self.approval_modal.is_open {
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
                    // Single canonical approval path (§8, §27): tool approvals
                    // resolve through the TUI bridge
                    // (ApplicationAction::ApprovalDecision → ApprovalCoordinator),
                    // never via a parallel RuntimeCommand dispatch. Session
                    // scope (AllowForSession) is deliberate here: the TUI
                    // governs an interactive session, while headless CLI
                    // ResolveApproval remains mission-scoped.
                    let sent = self
                        .bridge_tx
                        .as_ref()
                        .map(|tx| {
                            tx.send(ApplicationAction::ApprovalDecision {
                                request_id: approval_id.clone(),
                                decision,
                            })
                            .is_ok()
                        })
                        .unwrap_or(false);
                    if !sent {
                        self.model.add_conversation_item(
                            crate::tui::conversation::TuiConversationItem::Error {
                                message: "Cannot resolve approval: governed runtime bridge unavailable; approval blocked.".to_string(),
                                timestamp: chrono::Utc::now(),
                            },
                        );
                    }
                }
            }
            return None;
        }

        // 3. Command Palette handles keys if active
        if self.palette.is_open {
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
            return None;
        }

        // 4. Help Overlay handles keys if open
        if self.overlay_manager.is_help_open {
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
                return None;
            }
            return None;
        }

        // 5. Setup Wizard handles keys if open (View 22 - ModalDialog)
        if self.navigation.active_overlay == Some(ViewId::SetupWizard) {
            if self.focus.current() != FocusTarget::Overlay {
                self.focus.enter_overlay();
            }
            if let Some(ref mut wizard) = self.setup_wizard {
                let outcome = wizard.handle_key(key);
                self.model.mark_dirty();
                match outcome {
                    WizardOutcome::Completed => {
                        // Persist configuration and close wizard
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
            return None;
        }

        // Universal palette trigger (Ctrl+P) works anytime, even when composer is focused
        if (key.code == KeyCode::Char('p') || key.code == KeyCode::Char('P'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.palette.open();
            self.focus.enter_overlay();
            self.model.mark_dirty();
            return None;
        }

        // Universal interrupt trigger (Ctrl+C). Single canonical path (§8):
        // cancellation flows through the bridge (CancelRequested →
        // cancel_token + runtime.cancel_mission). No parallel
        // RuntimeCommand::CancelMission is emitted when the bridge owns the
        // session; direct dispatch survives only as a degraded fallback.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.bridge_tx.is_some() {
                self.model.settle_request();
                self.send_or_fail("cancel", ApplicationAction::CancelRequested);
                return None;
            }
            if self.model.mission_id.is_some() {
                return Some(RuntimeCommand::CancelMission {
                    id: self
                        .model
                        .mission_id
                        .clone()
                        .unwrap_or_else(|| "current".to_string()),
                    reason: Some("Cancelled via Ctrl+C (no runtime bridge)".to_string()),
                });
            }
            if self.is_composer_focused && !self.composer.text().is_empty() {
                self.composer.clear();
                self.model.mark_dirty();
                return None;
            }
        }

        // 5. Interactive Composer handles keys if focused
        if self.is_composer_focused {
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
                                let matches = active_q.options.iter().any(|o| {
                                    o.trim().eq_ignore_ascii_case(text.trim()) || o == &text
                                });
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
                            if let Some(c) = category
                                && let Some(_cat) =
                                    crate::tui::surface::settings::SettingsCategory::from_str_relaxed(
                                        &c,
                                    )
                            {
                                // Category pre-selection is handled by the
                                // settings shell state; navigation is the TUI
                                // side effect (projection only).
                            }
                            self.navigation.navigate_to(ScreenId::Settings);
                            self.navigation
                                .navigate_to_view(crate::tui::registry::ViewId::SettingsConfig);
                            self.model.settle_request();
                            return None;
                        }
                        ApplicationAction::DiffRequested => {
                            self.navigation.navigate_to(ScreenId::Git);
                            // Read-only inspection: no model activity owned.
                            self.send_or_fail("diff", ApplicationAction::DiffRequested);
                            return None;
                        }
                        ApplicationAction::ClearRequested => {
                            // Local projection cleared AND canonical
                            // durable clear requested: one honest path.
                            // Clearing settles any activity (nothing is
                            // "working" on an empty timeline).
                            self.model.conversation.clear();
                            self.model.settle_request();
                            self.send_or_fail("clear", ApplicationAction::ClearRequested);
                            return None;
                        }
                        ApplicationAction::ClearSessionRequested => {
                            self.model.conversation.clear();
                            self.model.settle_request();
                            self.send_or_fail(
                                "clear-session",
                                ApplicationAction::ClearSessionRequested,
                            );
                            return None;
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
                            return Some(RuntimeCommand::CancelMission {
                                id: self
                                    .model
                                    .mission_id
                                    .clone()
                                    .unwrap_or_else(|| "current".to_string()),
                                reason: Some(
                                    "Cancelled from composer (no runtime bridge)".to_string(),
                                ),
                            });
                        }
                        ApplicationAction::ExitRequested => {
                            self.is_running = false;
                            self.send_or_fail("exit", ApplicationAction::ExitRequested);
                            return None;
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
                        ApplicationAction::SlashCommandSubmitted { ref command, .. } => {
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
                                    self.navigation.navigate_to(ScreenId::Settings);
                                    self.navigation.navigate_to_view(
                                        crate::tui::registry::ViewId::SettingsConfig,
                                    );
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
                                    crate::interaction::commands::SlashCommandRegistry::new_standard(
                                    );
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
                            return None;
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
                            self.model.activity_message =
                                Some("Working on your request…".to_string());
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
                            return None;
                        }
                        other => {
                            self.send_or_fail("request", other);
                            return None;
                        }
                    }
                }
                ComposerAction::Cancel => {
                    self.unfocus_composer();
                    return None;
                }
                ComposerAction::None => return None,
            }
        }

        // 6. When composer is unfocused: focus cycling, activation shortcuts, scrolling, and navigation
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
                        if let Some(run_id) = self
                            .workflow_snapshot
                            .as_ref()
                            .map(|s| s.run.id.to_string())
                        {
                            self.send_or_fail(
                                "workflow resume",
                                ApplicationAction::WorkflowResumeRequested { run_id },
                            );
                        }
                    }
                    WorkflowAction::Pause => {
                        if let Some(run_id) = self
                            .workflow_snapshot
                            .as_ref()
                            .map(|s| s.run.id.to_string())
                        {
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
                        if let Some(run_id) = self
                            .workflow_snapshot
                            .as_ref()
                            .map(|s| s.run.id.to_string())
                        {
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
                        if let Some(run_id) = self
                            .workflow_snapshot
                            .as_ref()
                            .map(|s| s.run.id.to_string())
                        {
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

        let start = Instant::now();

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

        let elapsed = start.elapsed();
        self.last_render_duration_micros = elapsed.as_micros() as u64;
        self.frame_count += 1;
        self.model.spinner.tick();
        self.model.clear_dirty();
        self.force_redraw = false;

        Ok(true)
    }

    /// Step one tick of the decoupled TUI loop.
    pub fn tick<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<bool> {
        self.poll_updates();
        self.render_frame(terminal)
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
