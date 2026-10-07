//! Modular route composition (Principles 7–8).
//!
//! Each route composes M31A's existing surfaces — no business logic, no I/O,
//! no policy, no runtime calls. The previous giant `render_workspace()` match
//! is preserved pixel-for-pixel here as small per-route functions so screens
//! own their composition:
//!
//! ```text
//! Session = header + conversation + execution/tool activity
//!         + contextual inspector + composer + footer
//! ```
//!
//! Other routes keep M31A's visual design and responsive behavior unchanged
//! (conversation + inspector split on wide terminals, single surface on
//! narrow ones). `render_workspace()` in `shell/workspace.rs` remains the
//! thin orchestrator (startup → welcome → route → composer → rail → detail
//! → overlay) and delegates upper-area composition here.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::tui::composer::TuiComposer;
use crate::tui::focus::FocusTarget;
use crate::tui::model::TuiViewModel;
use crate::tui::navigation::ScreenId;
use crate::tui::replay::ReplayController;
use crate::tui::surface::{ModelSelectorState, render_model_selector};
use crate::tui::surface::{
    WorkflowDashboardState, render_agents_surface, render_artifacts_surface,
    render_conversation_surface, render_doctor_surface, render_git_surface, render_jobs_surface,
    render_replay_surface, render_tasks_surface, render_telemetry_surface, render_tools_surface,
    render_verification_surface,
};
use crate::tui::theme::ThemeTokens;

/// Everything a route needs to compose its upper area.
///
/// Bundles the previous 10+ argument list so routes stay readable without
/// changing what they render. All fields borrow; no ownership moves.
pub struct RouteContext<'a> {
    pub screen: ScreenId,
    pub focus: FocusTarget,
    pub context_selected_idx: usize,
    pub replay: &'a ReplayController,
    pub workflow_snapshot: &'a Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
    pub workflow_dashboard_state: &'a mut WorkflowDashboardState,
    pub model_selector_state: &'a mut ModelSelectorState,
}

impl<'a> RouteContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        screen: ScreenId,
        focus: FocusTarget,
        context_selected_idx: usize,
        replay: &'a ReplayController,
        workflow_snapshot: &'a Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
        workflow_dashboard_state: &'a mut WorkflowDashboardState,
        model_selector_state: &'a mut ModelSelectorState,
    ) -> Self {
        Self {
            screen,
            focus,
            context_selected_idx,
            replay,
            workflow_snapshot,
            workflow_dashboard_state,
            model_selector_state,
        }
    }
}

/// Compose the upper workspace area for the active route.
///
/// Pure composition over in-memory projection state. Never queries the
/// database, never executes commands, never calls the model.
pub fn render_route_upper(
    f: &mut Frame,
    upper_area: Rect,
    model: &mut TuiViewModel,
    _composer: &TuiComposer,
    ctx: &mut RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    // Historical threshold: wide (>= 100) splits conversation + inspector,
    // narrow renders the inspector surface alone. Identical for every route.
    match ctx.screen {
        ScreenId::Dashboard => render_session(f, upper_area, model, ctx, tokens),
        ScreenId::TaskGraph => render_tasks(f, upper_area, model, ctx, tokens),
        ScreenId::Agents => render_agents(f, upper_area, model, ctx, tokens),
        ScreenId::Tools => render_tools(f, upper_area, model, ctx, tokens),
        ScreenId::Git => render_git(f, upper_area, model, ctx, tokens),
        ScreenId::Verification => render_verification(f, upper_area, model, ctx, tokens),
        ScreenId::Jobs => render_jobs(f, upper_area, model, ctx, tokens),
        ScreenId::Doctor => render_doctor(f, upper_area, model, ctx, tokens),
        ScreenId::ModelUsage => render_models(f, upper_area, model, ctx, tokens),
        ScreenId::Artifacts => render_artifacts(f, upper_area, model, ctx, tokens),
        ScreenId::Replay => render_replay_route(f, upper_area, model, ctx, tokens),
        ScreenId::Mission
        | ScreenId::Approvals
        | ScreenId::Logs
        | ScreenId::Help
        | ScreenId::Settings => render_secondary(f, upper_area, model, ctx, tokens),
    }
}

fn wide_split(area: Rect, left_pct: u16, right_pct: u16) -> (Rect, Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(area);
    (cols[0], cols[1])
}

fn conversation_focused(ctx: &RouteContext<'_>) -> bool {
    ctx.focus == FocusTarget::Conversation
}

fn panel_focused(ctx: &RouteContext<'_>) -> bool {
    ctx.focus == FocusTarget::ContextPanel
}

/// Session route: the conversation IS the dashboard (M31A visual identity).
pub fn render_session(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    let convo_area = crate::tui::shell::workspace::constrain_reading_width(area, 120);
    render_conversation_surface(f, convo_area, model, tokens, conversation_focused(ctx));
}

#[allow(clippy::too_many_arguments)]
fn render_split_with_selected(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
    left_pct: u16,
    right_pct: u16,
    surface: fn(&mut Frame, Rect, &TuiViewModel, &ThemeTokens, bool, usize),
) {
    if area.width >= 100 {
        let (left, right) = wide_split(area, left_pct, right_pct);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        surface(
            f,
            right,
            model,
            tokens,
            panel_focused(ctx),
            ctx.context_selected_idx,
        );
    } else {
        surface(
            f,
            area,
            model,
            tokens,
            panel_focused(ctx),
            ctx.context_selected_idx,
        );
    }
}

pub fn render_tasks(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_split_with_selected(f, area, model, ctx, tokens, 45, 55, render_tasks_surface);
}

pub fn render_agents(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_split_with_selected(f, area, model, ctx, tokens, 45, 55, render_agents_surface);
}

pub fn render_tools(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_split_with_selected(f, area, model, ctx, tokens, 45, 55, render_tools_surface);
}

pub fn render_git(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    if area.width >= 100 {
        let (left, right) = wide_split(area, 40, 60);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        render_git_surface(
            f,
            right,
            model,
            tokens,
            panel_focused(ctx),
            ctx.context_selected_idx,
        );
    } else {
        render_git_surface(
            f,
            area,
            model,
            tokens,
            panel_focused(ctx),
            ctx.context_selected_idx,
        );
    }
}

pub fn render_verification(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_split_with_selected(
        f,
        area,
        model,
        ctx,
        tokens,
        45,
        55,
        render_verification_surface,
    );
}

pub fn render_jobs(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    if area.width >= 100 {
        let (left, right) = wide_split(area, 45, 55);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        render_jobs_surface(f, right, model, tokens, panel_focused(ctx));
    } else {
        render_jobs_surface(f, area, model, tokens, panel_focused(ctx));
    }
}

pub fn render_doctor(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    if area.width >= 100 {
        let (left, right) = wide_split(area, 45, 55);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        render_doctor_surface(f, right, model, tokens, panel_focused(ctx));
    } else {
        render_doctor_surface(f, area, model, tokens, panel_focused(ctx));
    }
}

pub fn render_models(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    if area.width >= 100 {
        let (left, right) = wide_split(area, 45, 55);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        render_telemetry_surface(f, right, model, tokens, panel_focused(ctx));
    } else {
        render_telemetry_surface(f, area, model, tokens, panel_focused(ctx));
    }
}

pub fn render_artifacts(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_split_with_selected(
        f,
        area,
        model,
        ctx,
        tokens,
        45,
        55,
        render_artifacts_surface,
    );
}

pub fn render_replay_route(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    render_replay_surface(f, area, model, ctx.replay, tokens, panel_focused(ctx));
}

/// Mission / Approvals / Logs / Help / Settings: conversation + secondary.
pub fn render_secondary(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    ctx: &RouteContext<'_>,
    tokens: &ThemeTokens,
) {
    let screen = ctx.screen;
    if area.width >= 100 {
        let (left, right) = wide_split(area, 50, 50);
        render_conversation_surface(f, left, model, tokens, conversation_focused(ctx));
        crate::tui::screens::render_screen(screen, f, right, model, ctx.replay);
    } else {
        crate::tui::screens::render_screen(screen, f, area, model, ctx.replay);
    }
}

/// Workflow dashboard detail affordance (DagInspector contextual strip).
///
/// Kept here so routes—not the orchestrator—own inspector composition. The
/// actual dashboard widget remains the existing M31A surface.
pub fn render_workflow_detail(
    f: &mut Frame,
    area: Rect,
    snapshot: &crate::workflow::engine::WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    focused: bool,
    state: &mut WorkflowDashboardState,
) {
    crate::tui::surface::render_workflow_dashboard(f, area, snapshot, tokens, focused, state);
}

/// Model selector detail affordance (ModelRegistry contextual strip).
pub fn render_model_selector_detail(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    state: &mut ModelSelectorState,
) {
    let (providers, models) =
        crate::tui::surface::model_selector::resolve_display_models_and_providers(
            &model.catalog_models,
            &model.active_provider,
            &model.active_model,
        );
    render_model_selector(
        f,
        area,
        &providers,
        &models,
        &model.active_provider,
        &model.active_model,
        None,
        tokens,
        true,
        state,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw_route(screen: ScreenId, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut model = TuiViewModel::new();
        model.enter_active_session();
        model.set_runtime_ready();
        let composer = TuiComposer::new(std::path::PathBuf::from("."));
        let replay = ReplayController::new();
        let tokens = crate::tui::theme::ThemeTokens::resolve(crate::tui::theme::ThemeMode::Default);
        let mut wf = WorkflowDashboardState::new();
        let mut ms = ModelSelectorState::new();
        terminal
            .draw(|f| {
                let area = f.area();
                let mut ctx = RouteContext::new(
                    screen,
                    FocusTarget::Conversation,
                    0,
                    &replay,
                    &None,
                    &mut wf,
                    &mut ms,
                );
                render_route_upper(f, area, &mut model, &composer, &mut ctx, &tokens);
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn every_route_composes_without_blank() {
        for screen in [
            ScreenId::Dashboard,
            ScreenId::TaskGraph,
            ScreenId::Agents,
            ScreenId::Tools,
            ScreenId::Git,
            ScreenId::Verification,
            ScreenId::Jobs,
            ScreenId::Doctor,
            ScreenId::ModelUsage,
            ScreenId::Artifacts,
            ScreenId::Replay,
            ScreenId::Mission,
            ScreenId::Approvals,
            ScreenId::Logs,
            ScreenId::Help,
            ScreenId::Settings,
        ] {
            let content = draw_route(screen, 120, 30);
            assert!(
                content.trim().chars().any(|c| !c.is_whitespace()),
                "{screen:?} route must not be blank"
            );
        }
    }
}
