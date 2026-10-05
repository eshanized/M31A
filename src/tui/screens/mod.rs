//! Operational Screen Renderers (TUI-01).
//!
//! Renders each of the operational screens defined by UI_UX_SPEC_M31A.md
//! as pure, zero-I/O projections over the in-memory `TuiViewModel`.

pub mod session_cockpit;
pub mod wizard;

pub use session_cockpit::render_session_cockpit;
pub use wizard::{SetupWizardScreen, WizardOutcome, WizardProfile};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::model::TuiViewModel;
use super::navigation::ScreenId;
use super::replay::ReplayController;
use super::surface::{
    render_agents_surface, render_artifacts_surface, render_doctor_surface, render_git_surface,
    render_jobs_surface, render_replay_surface, render_tasks_surface, render_telemetry_surface,
    render_tools_surface, render_verification_surface,
};
use super::theme::{ThemeMode, ThemeTokens};

/// Render widget for the currently active operational screen.
pub fn render_screen(
    screen: ScreenId,
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    replay: &ReplayController,
) {
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    match screen {
        ScreenId::Dashboard => render_dashboard(f, area, model, &tokens),
        ScreenId::Mission => render_mission(f, area, model, &tokens),
        ScreenId::TaskGraph => render_tasks_surface(f, area, model, &tokens, false, 0),
        ScreenId::Agents => render_agents_surface(f, area, model, &tokens, false, 0),
        ScreenId::Tools => render_tools_surface(f, area, model, &tokens, false, 0),
        ScreenId::Jobs => render_jobs_surface(f, area, model, &tokens, false),
        ScreenId::Verification => render_verification_surface(f, area, model, &tokens, false, 0),
        ScreenId::Git => render_git_surface(f, area, model, &tokens, false, 0),
        ScreenId::Approvals => render_approvals(f, area, model, &tokens),
        ScreenId::Doctor => render_doctor_surface(f, area, model, &tokens, false),
        ScreenId::Logs => render_logs(f, area, model, &tokens),
        ScreenId::ModelUsage => render_telemetry_surface(f, area, model, &tokens, false),
        ScreenId::Artifacts => render_artifacts_surface(f, area, model, &tokens, false, 0),
        ScreenId::Replay => render_replay_surface(f, area, model, replay, &tokens, false),
        ScreenId::Help => render_help(f, area, model, &tokens),
    }
}

fn render_dashboard(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    let mut text = format!(
        "Overview\n\
         ========================\n\
         Status:           {}\n\
         Objective:        {}\n\
         Tasks Total:      {} (Completed: {})\n\
         Active Agents:    {}\n\
         Events Processed: {}\n\
         Pending Approvals:{}\n\n\
         Recent Logs:\n",
        model.mission_status.to_uppercase(),
        model.objective,
        model.tasks.len(),
        model
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count(),
        model.agents.len(),
        model.system_stats.events_processed,
        model.approvals.len()
    );

    for log in model.logs.iter().rev().take(6) {
        text.push_str(&format!(
            " [{}] [{}] {}\n",
            log.level, log.source, log.message
        ));
    }

    let p = Paragraph::new(text).block(
        Block::default()
            .title(" Dashboard [1] ")
            .borders(Borders::NONE)
            .border_style(tokens.border_default),
    );
    f.render_widget(p, area);
}

fn render_mission(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    let lines = vec![
        Line::styled(
            "MISSION SPECIFICATION & INVARIANTS",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Mission ID  : ", tokens.text_muted),
            Span::styled(
                model.mission_id.as_deref().unwrap_or("none"),
                tokens.text_primary,
            ),
        ]),
        Line::from(vec![
            Span::styled("Name        : ", tokens.text_muted),
            Span::styled(
                &model.mission_name,
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Status      : ", tokens.text_muted),
            Span::styled(model.mission_status.to_uppercase(), tokens.status_running),
        ]),
        Line::from(vec![
            Span::styled("Objective   : ", tokens.text_muted),
            Span::styled(&model.objective, tokens.text_secondary),
        ]),
        Line::raw(""),
        Line::styled("Runtime Guarantees & Non-Negotiables:", tokens.text_muted),
        Line::styled(
            "  • Law 1: The model proposes. The runtime decides.",
            tokens.text_secondary,
        ),
        Line::styled(
            "  • Law 3: Every side effect passes through policy interception.",
            tokens.text_secondary,
        ),
        Line::styled(
            "  • Law 5: Never fake success; zero placeholder mocks.",
            tokens.text_secondary,
        ),
        Line::styled(
            "  • Law 6: Completion strictly requires independent verification evidence.",
            tokens.text_secondary,
        ),
        Line::styled(
            "  • Law 8: All long-running operations are cancellable via Ctrl+C / Esc.",
            tokens.text_secondary,
        ),
    ];

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Mission Detail [2] ")
                .borders(Borders::NONE)
                .border_style(tokens.border_default),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_approvals(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!("POLICY APPROVALS QUEUE ({} pending)", model.approvals.len()),
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    if model.approvals.is_empty() {
        lines.push(Line::styled(
            "  No pending approvals in authorization queue.",
            tokens.text_muted,
        ));
        lines.push(Line::styled(
            "  Mutating side effects requiring operator consent will appear here.",
            tokens.text_secondary,
        ));
    } else {
        for (i, req) in model.approvals.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}. [ASK] ", i + 1),
                    tokens.status_warning.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("Tool: {} ", req.tool_name),
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("(Agent: {}) ", req.agent_role),
                    tokens.accent_secondary,
                ),
                Span::styled(format!("[Risk: {}]", req.risk_tier), tokens.status_failed),
            ]));
            lines.push(Line::styled(
                format!("   ↳ Reason: {}", req.justification),
                tokens.text_secondary,
            ));
            lines.push(Line::styled(
                format!("   ↳ Params: {}", req.parameters_summary),
                tokens.text_muted,
            ));
            lines.push(Line::raw(""));
        }
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Approvals Queue [9] ")
                .borders(Borders::NONE)
                .border_style(tokens.border_default),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_logs(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    let mut lines = Vec::new();
    let max_lines = area.height.saturating_sub(2) as usize;

    for log in model.logs.iter().rev().take(max_lines) {
        let level_style = match log.level.as_str() {
            "ERROR" => tokens.status_failed,
            "WARN" => tokens.status_warning,
            "INFO" => tokens.status_ok,
            _ => tokens.text_muted,
        };

        let time_str = log.timestamp.format("%H:%M:%S").to_string();
        lines.push(Line::from(vec![
            Span::styled(format!("[{time_str}] "), tokens.text_muted),
            Span::styled(format!("[{:<5}] ", log.level), level_style),
            Span::styled(format!("[{:<10}] ", log.source), tokens.accent_primary),
            Span::styled(&log.message, tokens.text_primary),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::styled(
            "  No log events recorded in memory.",
            tokens.text_muted,
        ));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(format!(
                " Runtime Event Logs ({} entries) [L] ",
                model.logs.len()
            ))
            .borders(Borders::NONE)
            .border_style(tokens.border_default),
    );
    f.render_widget(p, area);
}

fn render_help(f: &mut Frame, area: Rect, _model: &TuiViewModel, tokens: &ThemeTokens) {
    crate::tui::overlay::help::render_help_overlay(f, area, tokens);
}
