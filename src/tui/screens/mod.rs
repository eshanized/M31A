//! TUI dialog screens.
//!
//! Route composition lives in [`crate::tui::routes`]; this module hosts only
//! the setup wizard dialog state machine. It owns no routing authority.

pub mod wizard;

pub use wizard::{SetupWizardScreen, WizardOutcome, WizardProfile};
