//! TUI dialog screens.
//!
//! Route composition lives in [`crate::tui::routes`]; this module hosts only
//! the setup wizard dialog state machine. It owns no routing authority.

pub mod wizard;
pub mod workspace_setup;

pub use wizard::{SetupWizardScreen, WizardOutcome, WizardProfile};
pub use workspace_setup::{
    DetectedRepoContext, GitIsolationChoice, PolicyPostureChoice, ProjectTypeChoice,
    WorkspaceSetupOutcome, WorkspaceSetupScreen, initialize_workspace_headless,
};
