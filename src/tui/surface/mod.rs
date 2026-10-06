//! Contextual Application Surfaces (Section 5, 6, 12–29).
//!
//! Replaces disconnected 40 screens with contextual surfaces, routes,
//! panels, and progressive master-detail projections.

pub mod agents;
pub mod artifacts;
pub mod conversation;
pub mod doctor;
pub mod git;
pub mod jobs;
pub mod model_selector;
pub mod replay;
pub mod settings;
pub mod tasks;
pub mod telemetry;
pub mod tools;
pub mod verification;
pub mod workflow;

pub use agents::render_agents_surface;
pub use artifacts::render_artifacts_surface;
pub use conversation::render_conversation_surface;
pub use doctor::render_doctor_surface;
pub use git::render_git_surface;
pub use jobs::render_jobs_surface;
pub use model_selector::{
    ModelInfo, ModelSelectorAction, ModelSelectorMode, ModelSelectorState, ProviderInfo,
    handle_model_selector_key, render_model_selector,
};
pub use replay::render_replay_surface;
pub use settings::{
    SettingsAction, SettingsCategory, SettingsRow, SettingsState, apply_edit_to_draft,
    handle_settings_key, mask_secret_configured, persist_workspace_config, render_settings,
    rows_for_category,
};
pub use tasks::render_tasks_surface;
pub use telemetry::render_telemetry_surface;
pub use tools::render_tools_surface;
pub use verification::render_verification_surface;
pub use workflow::{
    WorkflowAction, WorkflowDashboardState, WorkflowViewMode, handle_workflow_dashboard_key,
    render_workflow_dashboard,
};
