//! Contextual Application Surfaces (Section 5, 6, 12–29).
//!
//! Replaces disconnected 40 screens with contextual surfaces, routes,
//! panels, and progressive master-detail projections.

pub mod agents;
pub mod artifacts;
pub mod conversation;
pub mod decision;
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
pub use decision::render_decision_prompt;
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

/// Compute a visible range `(start_idx, end_idx)` for a selectable list of `total_items`,
/// guaranteeing that `selected_idx` is contained within the rendered viewport.
pub fn visible_list_range(
    total_items: usize,
    selected_idx: usize,
    viewport_height: usize,
) -> (usize, usize) {
    if total_items == 0 || viewport_height == 0 {
        return (0, 0);
    }
    let selected = selected_idx.min(total_items.saturating_sub(1));
    let viewport = viewport_height.max(1);

    if total_items <= viewport {
        return (0, total_items);
    }

    let start = if selected >= viewport {
        selected.saturating_sub(viewport - 1)
    } else {
        0
    };
    let end = (start + viewport).min(total_items);
    let start = if end == total_items {
        total_items.saturating_sub(viewport)
    } else {
        start
    };
    (start, end)
}
