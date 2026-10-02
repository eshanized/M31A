//! 40 Canonical Views Typed Registry (COP-03, D-05, UI_UX_SPEC §113).
//!
//! Strongly typed registry cataloging all 40 canonical operational views with
//! identity, numeric code (1–40), classification kind, shortcut hotkey, and URL/route path.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Canonical view identifiers across all 18 operational domains (A–R).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum ViewId {
    // Views 1–8: Core Cockpit & Observability
    MissionDashboard = 1,
    DagInspector = 2,
    TaskDetails = 3,
    AgentInspector = 4,
    ToolActivity = 5,
    ExecutionStream = 6,
    FailureRecovery = 7,
    SystemHealth = 8,

    // Views 9–16: Security, Policy, Sandbox & Audit
    PolicyLedger = 9,
    ApprovalsQueue = 10,
    ProcessSandbox = 11,
    AuditLog = 12,
    ArtifactExplorer = 13,
    GitTimeline = 14,
    TelemetryGraphs = 15,
    EventLog = 16,

    // Views 17–24: Operations, Skills, Profiles & Diagnostics
    BudgetMonitor = 17,
    ProfileMatrix = 18,
    SkillRegistry = 19,
    DoctorDiagnostics = 20,
    OnboardingTour = 21,
    SetupWizard = 22,
    WorkspaceSetup = 23,
    ModelRegistry = 24,

    // Views 25–32: Intelligence, Verification & Quarantines
    ProviderSetup = 25,
    PromptLab = 26,
    ContextMonitor = 27,
    CheckpointTree = 28,
    RollbackInspector = 29,
    QuarantineManager = 30,
    VerificationSuite = 31,
    ReportCard = 32,

    // Views 33–40: Configuration, Docs, Triage & Authoring
    PluginManager = 33,
    SettingsConfig = 34,
    KeybindingsGuide = 35,
    HelpDocs = 36,
    StartupRecovery = 37,
    CommandPalette = 38,
    DiffViewer = 39,
    MissionCreation = 40,
}

impl ViewId {
    /// Return view number 1 to 40.
    pub fn number(&self) -> u8 {
        *self as u8
    }

    /// Return all 40 canonical views in order.
    pub fn all() -> &'static [ViewId] {
        &[
            Self::MissionDashboard,
            Self::DagInspector,
            Self::TaskDetails,
            Self::AgentInspector,
            Self::ToolActivity,
            Self::ExecutionStream,
            Self::FailureRecovery,
            Self::SystemHealth,
            Self::PolicyLedger,
            Self::ApprovalsQueue,
            Self::ProcessSandbox,
            Self::AuditLog,
            Self::ArtifactExplorer,
            Self::GitTimeline,
            Self::TelemetryGraphs,
            Self::EventLog,
            Self::BudgetMonitor,
            Self::ProfileMatrix,
            Self::SkillRegistry,
            Self::DoctorDiagnostics,
            Self::OnboardingTour,
            Self::SetupWizard,
            Self::WorkspaceSetup,
            Self::ModelRegistry,
            Self::ProviderSetup,
            Self::PromptLab,
            Self::ContextMonitor,
            Self::CheckpointTree,
            Self::RollbackInspector,
            Self::QuarantineManager,
            Self::VerificationSuite,
            Self::ReportCard,
            Self::PluginManager,
            Self::SettingsConfig,
            Self::KeybindingsGuide,
            Self::HelpDocs,
            Self::StartupRecovery,
            Self::CommandPalette,
            Self::DiffViewer,
            Self::MissionCreation,
        ]
    }
}

/// Architectural classification of an operational view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    /// Top-level full-screen route in the primary cockpit flow.
    PrimaryRoute,
    /// Master-detail drilldown pane.
    DetailInspector,
    /// Focused dialog requiring user action or confirmation.
    ModalDialog,
    /// Transient floating overlay (command palette, search bar).
    Overlay,
}

/// Metadata descriptor for a canonical view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewMetadata {
    pub id: ViewId,
    pub number: u8,
    pub name: &'static str,
    pub kind: ViewKind,
    pub hotkey: Option<char>,
    pub route_path: &'static str,
    pub domain_category: &'static str,
}

type ViewDef = (
    ViewId,
    u8,
    &'static str,
    ViewKind,
    Option<char>,
    &'static str,
    &'static str,
);

fn cockpit_views() -> [ViewDef; 8] {
    [
        (
            ViewId::MissionDashboard,
            1,
            "Mission Dashboard",
            ViewKind::PrimaryRoute,
            Some('1'),
            "/cockpit/dashboard",
            "Cockpit",
        ),
        (
            ViewId::DagInspector,
            2,
            "DAG Inspector",
            ViewKind::PrimaryRoute,
            Some('2'),
            "/cockpit/dag",
            "Planning",
        ),
        (
            ViewId::TaskDetails,
            3,
            "Task Details",
            ViewKind::DetailInspector,
            Some('3'),
            "/cockpit/tasks",
            "Execution",
        ),
        (
            ViewId::AgentInspector,
            4,
            "Agent Inspector",
            ViewKind::DetailInspector,
            Some('4'),
            "/cockpit/agents",
            "Swarm",
        ),
        (
            ViewId::ToolActivity,
            5,
            "Tool Activity",
            ViewKind::DetailInspector,
            Some('5'),
            "/cockpit/tools",
            "Capabilities",
        ),
        (
            ViewId::ExecutionStream,
            6,
            "Execution Stream",
            ViewKind::PrimaryRoute,
            Some('6'),
            "/cockpit/stream",
            "Execution",
        ),
        (
            ViewId::FailureRecovery,
            7,
            "Failure Recovery",
            ViewKind::DetailInspector,
            Some('7'),
            "/cockpit/recovery",
            "Reliability",
        ),
        (
            ViewId::SystemHealth,
            8,
            "System Health",
            ViewKind::DetailInspector,
            Some('8'),
            "/cockpit/health",
            "Diagnostics",
        ),
    ]
}

fn security_views() -> [ViewDef; 8] {
    [
        (
            ViewId::PolicyLedger,
            9,
            "Policy Ledger",
            ViewKind::PrimaryRoute,
            Some('9'),
            "/security/policies",
            "Security",
        ),
        (
            ViewId::ApprovalsQueue,
            10,
            "Approvals Queue",
            ViewKind::ModalDialog,
            Some('0'),
            "/security/approvals",
            "Security",
        ),
        (
            ViewId::ProcessSandbox,
            11,
            "Process Sandbox",
            ViewKind::DetailInspector,
            None,
            "/security/sandbox",
            "Isolation",
        ),
        (
            ViewId::AuditLog,
            12,
            "Audit Log",
            ViewKind::PrimaryRoute,
            None,
            "/security/audit",
            "Audit",
        ),
        (
            ViewId::ArtifactExplorer,
            13,
            "Artifact Explorer",
            ViewKind::PrimaryRoute,
            Some('a'),
            "/artifacts/explorer",
            "Artifacts",
        ),
        (
            ViewId::GitTimeline,
            14,
            "Git Timeline",
            ViewKind::PrimaryRoute,
            Some('g'),
            "/git/timeline",
            "VersionControl",
        ),
        (
            ViewId::TelemetryGraphs,
            15,
            "Telemetry Graphs",
            ViewKind::PrimaryRoute,
            None,
            "/telemetry/graphs",
            "Observability",
        ),
        (
            ViewId::EventLog,
            16,
            "Event Log",
            ViewKind::PrimaryRoute,
            Some('l'),
            "/telemetry/events",
            "Observability",
        ),
    ]
}

fn operations_views() -> [ViewDef; 8] {
    [
        (
            ViewId::BudgetMonitor,
            17,
            "Budget Monitor",
            ViewKind::PrimaryRoute,
            Some('b'),
            "/ops/budget",
            "Operations",
        ),
        (
            ViewId::ProfileMatrix,
            18,
            "Profile Matrix",
            ViewKind::DetailInspector,
            None,
            "/ops/profiles",
            "Profiles",
        ),
        (
            ViewId::SkillRegistry,
            19,
            "Skill Registry",
            ViewKind::DetailInspector,
            Some('s'),
            "/ops/skills",
            "Skills",
        ),
        (
            ViewId::DoctorDiagnostics,
            20,
            "Doctor Diagnostics",
            ViewKind::PrimaryRoute,
            Some('d'),
            "/ops/doctor",
            "Diagnostics",
        ),
        (
            ViewId::OnboardingTour,
            21,
            "Onboarding Tour",
            ViewKind::Overlay,
            None,
            "/onboarding/tour",
            "Onboarding",
        ),
        (
            ViewId::SetupWizard,
            22,
            "Setup Wizard",
            ViewKind::ModalDialog,
            None,
            "/onboarding/wizard",
            "Onboarding",
        ),
        (
            ViewId::WorkspaceSetup,
            23,
            "Workspace Setup",
            ViewKind::DetailInspector,
            Some('w'),
            "/onboarding/workspace",
            "Workspace",
        ),
        (
            ViewId::ModelRegistry,
            24,
            "Model Registry",
            ViewKind::DetailInspector,
            Some('m'),
            "/intelligence/models",
            "Intelligence",
        ),
    ]
}

fn intelligence_views() -> [ViewDef; 8] {
    [
        (
            ViewId::ProviderSetup,
            25,
            "Provider Setup",
            ViewKind::DetailInspector,
            None,
            "/intelligence/providers",
            "Intelligence",
        ),
        (
            ViewId::PromptLab,
            26,
            "Prompt Lab",
            ViewKind::PrimaryRoute,
            None,
            "/intelligence/prompt-lab",
            "Intelligence",
        ),
        (
            ViewId::ContextMonitor,
            27,
            "Context Monitor",
            ViewKind::DetailInspector,
            None,
            "/intelligence/context",
            "Intelligence",
        ),
        (
            ViewId::CheckpointTree,
            28,
            "Checkpoint Tree",
            ViewKind::DetailInspector,
            Some('c'),
            "/reliability/checkpoints",
            "Reliability",
        ),
        (
            ViewId::RollbackInspector,
            29,
            "Rollback Inspector",
            ViewKind::ModalDialog,
            None,
            "/reliability/rollback",
            "Reliability",
        ),
        (
            ViewId::QuarantineManager,
            30,
            "Quarantine Manager",
            ViewKind::DetailInspector,
            None,
            "/reliability/quarantine",
            "Reliability",
        ),
        (
            ViewId::VerificationSuite,
            31,
            "Verification Suite",
            ViewKind::PrimaryRoute,
            Some('v'),
            "/quality/verification",
            "Quality",
        ),
        (
            ViewId::ReportCard,
            32,
            "Report Card",
            ViewKind::PrimaryRoute,
            None,
            "/quality/report",
            "Quality",
        ),
    ]
}

fn configuration_views() -> [ViewDef; 8] {
    [
        (
            ViewId::PluginManager,
            33,
            "Plugin Manager",
            ViewKind::DetailInspector,
            None,
            "/system/plugins",
            "Extensibility",
        ),
        (
            ViewId::SettingsConfig,
            34,
            "Settings & Config",
            ViewKind::PrimaryRoute,
            None,
            "/system/settings",
            "Configuration",
        ),
        (
            ViewId::KeybindingsGuide,
            35,
            "Keybindings Guide",
            ViewKind::Overlay,
            Some('k'),
            "/help/keybindings",
            "Help",
        ),
        (
            ViewId::HelpDocs,
            36,
            "Help & Docs",
            ViewKind::Overlay,
            Some('?'),
            "/help/docs",
            "Help",
        ),
        (
            ViewId::StartupRecovery,
            37,
            "Startup Recovery",
            ViewKind::ModalDialog,
            None,
            "/recovery/startup",
            "Recovery",
        ),
        (
            ViewId::CommandPalette,
            38,
            "Command Palette",
            ViewKind::Overlay,
            Some(':'),
            "/overlay/palette",
            "Navigation",
        ),
        (
            ViewId::DiffViewer,
            39,
            "Diff Viewer",
            ViewKind::DetailInspector,
            None,
            "/git/diff",
            "VersionControl",
        ),
        (
            ViewId::MissionCreation,
            40,
            "Mission Creation",
            ViewKind::ModalDialog,
            Some('n'),
            "/mission/create",
            "Authoring",
        ),
    ]
}

/// Central registry of all 40 canonical views.
#[derive(Debug, Clone)]
pub struct ViewRegistry {
    views: HashMap<ViewId, ViewMetadata>,
    by_number: HashMap<u8, ViewId>,
    by_route: HashMap<&'static str, ViewId>,
}

impl Default for ViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewRegistry {
    /// Build registry and populate all 40 views.
    pub fn new() -> Self {
        let mut reg = Self {
            views: HashMap::new(),
            by_number: HashMap::new(),
            by_route: HashMap::new(),
        };

        reg.register_views(&cockpit_views());
        reg.register_views(&security_views());
        reg.register_views(&operations_views());
        reg.register_views(&intelligence_views());
        reg.register_views(&configuration_views());

        reg
    }

    fn register_views(&mut self, defs: &[ViewDef]) {
        for &(id, number, name, kind, hotkey, route_path, domain_category) in defs {
            let meta = ViewMetadata {
                id,
                number,
                name,
                kind,
                hotkey,
                route_path,
                domain_category,
            };
            self.views.insert(id, meta);
            self.by_number.insert(number, id);
            self.by_route.insert(route_path, id);
        }
    }

    /// Return total registered view count (strictly 40 per UI_UX_SPEC §113).
    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// Retrieve metadata by ViewId.
    pub fn get(&self, id: ViewId) -> Option<&'_ ViewMetadata> {
        self.views.get(&id)
    }

    /// Retrieve metadata by 1-based number (1 to 40).
    pub fn get_by_number(&self, number: u8) -> Option<&'_ ViewMetadata> {
        self.by_number
            .get(&number)
            .and_then(|id| self.views.get(id))
    }

    /// Retrieve metadata by route path.
    pub fn get_by_route(&self, route: &str) -> Option<&'_ ViewMetadata> {
        self.by_route.get(route).and_then(|id| self.views.get(id))
    }

    /// Find view by shortcut hotkey.
    pub fn get_by_hotkey(&self, hotkey: char) -> Option<&'_ ViewMetadata> {
        self.views.values().find(|v| v.hotkey == Some(hotkey))
    }

    /// Search views by substring name match (case-insensitive).
    pub fn find_by_name(&self, query: &str) -> Vec<&'_ ViewMetadata> {
        let q = query.to_lowercase();
        let mut matches: Vec<&'_ ViewMetadata> = self
            .views
            .values()
            .filter(|v| {
                v.name.to_lowercase().contains(&q) || v.domain_category.to_lowercase().contains(&q)
            })
            .collect();
        matches.sort_by_key(|v| v.number);
        matches
    }

    /// Return all 40 view metadata records sorted by number.
    pub fn all(&self) -> Vec<&'_ ViewMetadata> {
        let mut all: Vec<&'_ ViewMetadata> = self.views.values().collect();
        all.sort_by_key(|v| v.number);
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_contains_all_40_views() {
        let registry = ViewRegistry::new();
        assert_eq!(registry.len(), 40);
        assert!(!registry.is_empty());

        for i in 1..=40 {
            let view = registry.get_by_number(i);
            assert!(view.is_some(), "View number {} missing", i);
            let view = view.unwrap();
            assert_eq!(view.number, i);
        }
    }

    #[test]
    fn test_view_id_all_matches_registry() {
        let registry = ViewRegistry::new();
        assert_eq!(ViewId::all().len(), 40);
        for &id in ViewId::all() {
            let view = registry.get(id);
            assert!(view.is_some(), "ViewId {:?} missing in registry", id);
            assert_eq!(view.unwrap().id, id);
        }
    }

    #[test]
    fn test_route_lookups() {
        let registry = ViewRegistry::new();
        let dashboard = registry.get_by_route("/cockpit/dashboard").unwrap();
        assert_eq!(dashboard.id, ViewId::MissionDashboard);
        assert_eq!(dashboard.number, 1);

        let mission_creation = registry.get_by_route("/mission/create").unwrap();
        assert_eq!(mission_creation.id, ViewId::MissionCreation);
        assert_eq!(mission_creation.number, 40);
    }

    #[test]
    fn test_hotkey_and_search_lookups() {
        let registry = ViewRegistry::new();
        let dashboard = registry.get_by_hotkey('1').unwrap();
        assert_eq!(dashboard.id, ViewId::MissionDashboard);

        let search_results = registry.find_by_name("Dashboard");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].id, ViewId::MissionDashboard);
    }
}
