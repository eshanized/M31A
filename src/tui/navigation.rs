//! Canonical Navigation & Shortcut Router (TUI-01, D-16).
//!
//! Maps global canonical keyboard shortcuts:
//! - `1`–`0`: Primary operational screens (Dashboard, Mission, Task Graph, Agents, Tools, Jobs, Verification, Git, Approvals, Doctor)
//! - `l`, `m`, `a`, `r`, `?`: Auxiliary screens (Logs, Model, Artifacts, Replay, Help)
//! - `Ctrl+P` or `:`: Open universal fuzzy Command Palette
//! - `Esc`: Pop navigation history stack or close overlay

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

use crate::tui::registry::{ViewId, ViewKind, ViewRegistry};

/// Operational screens defined by UI_UX_SPEC_M31A.md and TUI-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenId {
    Dashboard,
    Mission,
    TaskGraph,
    Agents,
    Tools,
    Jobs,
    Verification,
    Git,
    Approvals,
    Doctor,
    Logs,
    ModelUsage,
    Artifacts,
    Replay,
    Help,
    Settings,
}

impl ScreenId {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Dashboard => "Conversation",
            Self::Mission => "Mission",
            Self::TaskGraph => "Tasks",
            Self::Agents => "Agents",
            Self::Tools => "Tools",
            Self::Jobs => "Jobs",
            Self::Verification => "Verification",
            Self::Git => "Git",
            Self::Approvals => "Approvals",
            Self::Doctor => "Doctor",
            Self::Logs => "Logs",
            Self::ModelUsage => "Model usage",
            Self::Artifacts => "Artifacts",
            Self::Replay => "Replay",
            Self::Help => "Help",
            Self::Settings => "Settings",
        }
    }

    /// Human-readable title with shortcut hint for help/palette contexts.
    /// Panel chrome must use `title()` (no shortcut noise); this helper is
    /// only for places that explicitly document keybindings.
    pub fn title_with_shortcut(&self) -> &'static str {
        match self {
            Self::Dashboard => "Conversation [1]",
            Self::Mission => "Mission [6]",
            Self::TaskGraph => "Tasks [2]",
            Self::Agents => "Agents [4]",
            Self::Tools => "Tools [5]",
            Self::Jobs => "Jobs",
            Self::Verification => "Verification [v]",
            Self::Git => "Git [g]",
            Self::Approvals => "Approvals [9/0]",
            Self::Doctor => "Doctor [8/d]",
            Self::Logs => "Logs [L]",
            Self::ModelUsage => "Model usage [M]",
            Self::Artifacts => "Artifacts [A]",
            Self::Replay => "Replay",
            Self::Help => "Help [?]",
            Self::Settings => "Settings [/settings]",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Dashboard => "1",
            Self::Mission => "6",
            Self::TaskGraph => "2",
            Self::Agents => "4",
            Self::Tools => "5",
            Self::Jobs => "",
            Self::Verification => "v",
            Self::Git => "g",
            Self::Approvals => "9",
            Self::Doctor => "d",
            Self::Logs => "l",
            Self::ModelUsage => "m",
            Self::Artifacts => "a",
            Self::Replay => "",
            Self::Help => "?",
            Self::Settings => "s",
        }
    }
}

/// Action resulting from evaluating a user keyboard event.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationAction {
    ScreenChanged(ScreenId),
    OpenPalette,
    TogglePause,
    Quit,
    None,
}

/// How a canonical `ViewId` resolves against the single navigation authority.
///
/// Primary routes navigate the main workspace. Detail inspectors and overlays
/// open contextually on top of their parent route instead of becoming
/// disconnected top-level screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteResolution {
    PrimaryRoute(ScreenId),
    DetailInspector { parent: ScreenId, detail: ViewId },
    ModalDialog { parent: ScreenId, dialog: ViewId },
    Overlay { parent: ScreenId, overlay: ViewId },
}

/// Canonical parent screen for every registered view.
///
/// This is the single mapping authority from the 40-view registry to the
/// workspace routes. Every `ViewId` resolves to a functional parent screen;
/// nothing is silently dropped.
pub fn canonical_screen(view: ViewId) -> ScreenId {
    match view {
        ViewId::MissionDashboard => ScreenId::Dashboard,
        ViewId::DagInspector | ViewId::TaskDetails => ScreenId::TaskGraph,
        ViewId::AgentInspector => ScreenId::Agents,
        ViewId::ToolActivity => ScreenId::Tools,
        ViewId::ExecutionStream | ViewId::FailureRecovery => ScreenId::Mission,
        ViewId::SystemHealth | ViewId::DoctorDiagnostics => ScreenId::Doctor,
        ViewId::PolicyLedger | ViewId::ApprovalsQueue => ScreenId::Approvals,
        ViewId::ProcessSandbox => ScreenId::Jobs,
        ViewId::AuditLog | ViewId::EventLog => ScreenId::Logs,
        ViewId::ArtifactExplorer | ViewId::ReportCard => ScreenId::Artifacts,
        ViewId::GitTimeline | ViewId::DiffViewer => ScreenId::Git,
        ViewId::TelemetryGraphs
        | ViewId::BudgetMonitor
        | ViewId::ModelRegistry
        | ViewId::ContextMonitor => ScreenId::ModelUsage,
        ViewId::ProfileMatrix | ViewId::SkillRegistry | ViewId::PluginManager => {
            ScreenId::ModelUsage
        }
        ViewId::VerificationSuite | ViewId::CheckpointTree | ViewId::RollbackInspector => {
            ScreenId::Verification
        }
        ViewId::QuarantineManager | ViewId::StartupRecovery => ScreenId::Doctor,
        ViewId::OnboardingTour
        | ViewId::SetupWizard
        | ViewId::WorkspaceSetup
        | ViewId::ProviderSetup => ScreenId::Doctor,
        ViewId::SettingsConfig => ScreenId::Settings,
        ViewId::PromptLab => ScreenId::Mission,
        ViewId::KeybindingsGuide | ViewId::HelpDocs => ScreenId::Help,
        ViewId::CommandPalette | ViewId::MissionCreation => ScreenId::Dashboard,
    }
}

/// Resolve a canonical view into a route plus its contextual presentation.
///
/// Primary routes navigate directly. All other kinds open contextually on
/// their parent screen, preserving the conversation-first session context.
pub fn resolve_view(view: ViewId) -> RouteResolution {
    let parent = canonical_screen(view);
    let kind = ViewRegistry::new()
        .get(view)
        .map(|m| m.kind)
        .unwrap_or(ViewKind::DetailInspector);
    match kind {
        ViewKind::PrimaryRoute => RouteResolution::PrimaryRoute(parent),
        ViewKind::DetailInspector => RouteResolution::DetailInspector {
            parent,
            detail: view,
        },
        ViewKind::ModalDialog => RouteResolution::ModalDialog {
            parent,
            dialog: view,
        },
        ViewKind::Overlay => RouteResolution::Overlay {
            parent,
            overlay: view,
        },
    }
}

/// Navigation router managing active screen, history stack, and keyboard shortcuts.
///
/// This is the single canonical navigation authority for the TUI. It owns the
/// primary route plus optional contextual detail and overlay views opened from
/// the 40-view registry. The legacy `router::NavigationRouter` is a
/// compatibility shim and must not be used for new routing decisions.
#[derive(Debug, Clone)]
pub struct NavigationRouter {
    pub current_screen: ScreenId,
    pub history: Vec<ScreenId>,
    pub max_history: usize,
    /// Contextual detail inspector opened on top of the current route.
    pub active_detail: Option<ViewId>,
    /// Focused dialog requiring user action or confirmation.
    pub active_modal: Option<ViewId>,
    /// Transient overlay opened on top of the current route.
    pub active_overlay: Option<ViewId>,
}

impl Default for NavigationRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationRouter {
    pub fn new() -> Self {
        Self {
            current_screen: ScreenId::Dashboard,
            history: Vec::new(),
            max_history: 32,
            active_detail: None,
            active_modal: None,
            active_overlay: None,
        }
    }

    /// Resolve a canonical view through the single navigation authority.
    ///
    /// Primary routes navigate the workspace; detail inspectors, modal
    /// dialogs, and overlays open contextually without leaving the session.
    pub fn navigate_to_view(&mut self, view: ViewId) {
        match resolve_view(view) {
            RouteResolution::PrimaryRoute(screen) => {
                self.close_modal();
                self.close_overlay();
                self.close_detail();
                self.navigate_to(screen);
            }
            RouteResolution::DetailInspector { parent, detail } => {
                self.close_modal();
                self.close_overlay();
                self.navigate_to(parent);
                self.open_detail(detail);
            }
            RouteResolution::ModalDialog { parent, dialog } => {
                self.close_overlay();
                self.navigate_to(parent);
                self.open_modal(dialog);
            }
            RouteResolution::Overlay { parent, overlay } => {
                self.close_modal();
                self.navigate_to(parent);
                self.open_overlay(overlay);
            }
        }
    }

    /// Open a modal dialog on the current route.
    pub fn open_modal(&mut self, modal: ViewId) {
        self.active_modal = Some(modal);
        // Only mirror to active_overlay if no overlay is currently active
        if self.active_overlay.is_none() {
            self.active_overlay = Some(modal);
        }
    }

    /// Close the active modal dialog.
    pub fn close_modal(&mut self) {
        let prev = self.active_modal.take();
        if self.active_overlay == prev {
            self.active_overlay = None;
        }
    }

    /// Open a contextual detail inspector on the current route.
    pub fn open_detail(&mut self, detail: ViewId) {
        self.active_detail = Some(detail);
    }

    /// Close the active detail inspector.
    pub fn close_detail(&mut self) {
        self.active_detail = None;
    }

    /// Open a transient overlay on the current route.
    pub fn open_overlay(&mut self, overlay: ViewId) {
        self.active_overlay = Some(overlay);
    }

    /// Close the active overlay.
    pub fn close_overlay(&mut self) {
        self.active_overlay = None;
    }

    /// Navigate to target screen, pushing previous screen onto history stack.
    pub fn navigate_to(&mut self, target: ScreenId) {
        if self.current_screen != target {
            if self.history.len() >= self.max_history {
                self.history.remove(0);
            }
            self.history.push(self.current_screen);
            self.current_screen = target;
        }
    }

    /// Return to previous screen from history stack with strict hierarchical dismissal:
    /// Modal -> Overlay -> Detail -> History Stack.
    pub fn pop_history(&mut self) -> Option<ScreenId> {
        if let Some(modal) = self.active_modal.take() {
            if self.active_overlay == Some(modal) {
                self.active_overlay = None;
            }
            return Some(self.current_screen);
        }
        if self.active_overlay.is_some() {
            self.active_overlay = None;
            return Some(self.current_screen);
        }
        if self.active_detail.is_some() {
            self.active_detail = None;
            return Some(self.current_screen);
        }
        if let Some(prev) = self.history.pop() {
            self.current_screen = prev;
            Some(prev)
        } else {
            None
        }
    }

    /// Evaluate key event into a navigation action through the canonical ViewRegistry.
    pub fn handle_key(&mut self, key: KeyEvent) -> NavigationAction {
        // 1. Check for Command Palette triggers: Ctrl+P or ':'
        if (key.code == KeyCode::Char('p') || key.code == KeyCode::Char('P'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return NavigationAction::OpenPalette;
        }
        if key.code == KeyCode::Char(':') && key.modifiers.is_empty() {
            return NavigationAction::OpenPalette;
        }

        // 2. Check for Escape (pop history)
        if key.code == KeyCode::Esc {
            if let Some(prev) = self.pop_history() {
                return NavigationAction::ScreenChanged(prev);
            }
            return NavigationAction::None;
        }

        // 3. Space toggles pause
        if key.code == KeyCode::Char(' ') {
            return NavigationAction::TogglePause;
        }

        // 4. 'q' quits
        if key.code == KeyCode::Char('q')
            && self.active_modal.is_none()
            && self.active_overlay.is_none()
            && self.active_detail.is_none()
        {
            return NavigationAction::Quit;
        }

        // 5. Canonical registry hotkeys (deterministic single authority)
        if let KeyCode::Char(ch) = key.code {
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
            {
                let registry = ViewRegistry::new();
                let matched = registry.get_by_hotkey(ch).or_else(|| {
                    if ch.is_uppercase() {
                        registry.get_by_hotkey(ch.to_ascii_lowercase())
                    } else {
                        None
                    }
                });

                if let Some(meta) = matched {
                    self.navigate_to_view(meta.id);
                    return NavigationAction::ScreenChanged(self.current_screen);
                }
            }
        }

        NavigationAction::None
    }
}
