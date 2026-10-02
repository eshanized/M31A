//! Persistent Application Shell Header (Section 5, 12, TUI-01).
//!
//! Delivers the persistent status strip across all modes and screens:
//! - Mission status, active task progress, objective
//! - Model provider, token usage, runtime cost
//! - Git branch and worktree state
//! - Replay mode warning banner when in post-mortem

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::model::TuiViewModel;
use crate::tui::navigation::ScreenId;
use crate::tui::replay::ReplayController;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the persistent application shell header.
pub fn render_header(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    screen: ScreenId,
    replay: &ReplayController,
    tokens: &ThemeTokens,
) {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();

    if replay.is_active {
        let text = format!(
            " [REPLAY MODE - READ ONLY] Frame {}/{} | Speed: {} | [ [ / ] ] Step | Space Play/Pause | Esc Exit",
            replay.cursor + 1,
            replay.history.len(),
            replay.speed.as_str()
        );
        let header = Paragraph::new(text)
            .style(if is_mono {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            })
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(header, area);
        return;
    }

    let is_executing = model.mission_status == "executing";
    let is_running = is_executing
        || model.session_status == "running"
        || model.mission_status == "running"
        || model.session_status == "active";

    let status_str = if is_executing {
        "EXECUTING"
    } else if is_running {
        "RUNNING"
    } else if model.mission_status == "completed" {
        "COMPLETED"
    } else if model.mission_status == "failed" {
        "FAILED"
    } else if model.mission_status == "paused" {
        "PAUSED"
    } else {
        "READY"
    };

    let status_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else if is_running {
        tokens.status_running.add_modifier(Modifier::BOLD)
    } else if status_str == "COMPLETED" {
        tokens.status_ok.add_modifier(Modifier::BOLD)
    } else if status_str == "FAILED" {
        tokens.status_failed.add_modifier(Modifier::BOLD)
    } else {
        tokens.accent_primary.add_modifier(Modifier::BOLD)
    };

    let _sid = model.session_id.as_deref().unwrap_or("standalone");
    let _mid = model.mission_id.as_deref().unwrap_or("none");
    let branch_display = match (&model.execution_worktree_branch, model.git_branch.as_str()) {
        (Some(exec_branch), ws) if !ws.is_empty() && ws != "N/A" && exec_branch != ws => {
            format!("{exec_branch} (ws: {ws})")
        }
        (Some(exec_branch), _) => exec_branch.clone(),
        (None, "") => "N/A".to_string(),
        (None, ws) => ws.to_string(),
    };

    let model_name = if model.active_model.is_empty() || model.active_model == "none" {
        "default"
    } else {
        &model.active_model
    };
    let provider_display = if model.active_provider.is_empty() || model.active_provider == "none" {
        String::new()
    } else {
        format!(" ({})", model.active_provider)
    };

    let tok_count = model.model_usage.prompt_tokens + model.model_usage.completion_tokens;
    let tok_str = if tok_count >= 1000 {
        format!("{:.1}k tok", tok_count as f64 / 1000.0)
    } else {
        format!("{tok_count} tok")
    };
    let cost_str = match model.model_usage.total_cost_cents {
        Some(cents) => format!("${:.2}", cents as f64 / 100.0),
        None => "cost n/a".to_string(),
    };
    let elapsed_str = format!("{}s", model.system_stats.uptime_secs);

    // Deployment identity (compile-time, no I/O): unobtrusive version +
    // channel label. Detailed build metadata lives in `m31a doctor` and
    // `m31a version --verbose`, never in the normal cockpit header.
    let deployment = crate::deployment::DeploymentContext::current();
    let deployment_label = deployment.cockpit_label();

    let lines = if area.height >= 3 {
        // Multi-line header for standard/large/ultrawide viewports
        let mut row0: Vec<Span> = Vec::new();
        row0.push(Span::styled(
            " M31A Cockpit ",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ));
        row0.push(Span::styled(
            deployment_label.as_str(),
            tokens.text_secondary,
        ));
        row0.push(Span::raw("| "));
        row0.push(Span::styled(
            screen.title(),
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ));
        row0.push(Span::raw(" | Status: "));
        row0.push(Span::styled(format!("[{status_str}]"), status_style));
        // Governed lifecycle state is first-class in the header so the
        // operator always sees what the runtime is waiting for.
        if model.lifecycle.stage != crate::tui::lifecycle::TuiLifecycleStage::Idle {
            row0.push(Span::raw(" | Lifecycle: "));
            row0.push(Span::styled(model.lifecycle.stage.label(), status_style));
        }
        if !model.objective.is_empty() {
            row0.push(Span::raw(" | Objective: "));
            row0.push(Span::styled(&model.objective, tokens.text_secondary));
        }

        let mut row1: Vec<Span> = Vec::new();
        if is_running && !model.tasks.is_empty() {
            let total = model.tasks.len();
            let completed = model
                .tasks
                .iter()
                .filter(|t| t.status == "completed")
                .count();
            let active_title = model
                .tasks
                .iter()
                .find(|t| t.status == "running" || t.status == "in_progress")
                .map(|t| t.title.as_str())
                .unwrap_or("executing");
            let task_idx = (completed + 1).min(total);

            row1.push(Span::raw("  "));
            row1.push(Span::styled("● ", status_style));
            row1.push(Span::styled(
                format!("Task {task_idx}/{total} ({active_title}) "),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            row1.push(Span::raw("· "));
            row1.push(Span::raw(format!(
                "{elapsed_str} · {tok_str} ({cost_str}) "
            )));
            row1.push(Span::raw("| Model: "));
            row1.push(Span::styled(
                format!("{model_name}{provider_display}"),
                Style::default().fg(if is_mono { Color::White } else { Color::Yellow }),
            ));
            row1.push(Span::raw(" | Branch: "));
            row1.push(Span::styled(&branch_display, tokens.status_ok));
        } else {
            row1.push(Span::raw("  Model: "));
            row1.push(Span::styled(
                format!("{model_name}{provider_display}"),
                Style::default().fg(if is_mono { Color::White } else { Color::Yellow }),
            ));
            row1.push(Span::raw(" · "));
            row1.push(Span::raw(format!(
                "{elapsed_str} · {tok_str} ({cost_str}) "
            )));
            row1.push(Span::raw("| Branch: "));
            row1.push(Span::styled(&branch_display, tokens.status_ok));
        }

        vec![Line::from(row0), Line::from(row1)]
    } else {
        // Compact single-line header for 80x24 viewports
        let mut row0: Vec<Span> = Vec::new();
        row0.push(Span::styled(
            " M31A Cockpit ",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ));
        row0.push(Span::styled(
            deployment_label.as_str(),
            tokens.text_secondary,
        ));
        row0.push(Span::raw("| "));
        row0.push(Span::styled(
            screen.title(),
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ));
        row0.push(Span::raw(" | "));
        row0.push(Span::styled(format!("[{status_str}]"), status_style));
        if model.lifecycle.stage != crate::tui::lifecycle::TuiLifecycleStage::Idle {
            row0.push(Span::raw(" "));
            row0.push(Span::styled(model.lifecycle.stage.label(), status_style));
        }

        if is_running && !model.tasks.is_empty() {
            let total = model.tasks.len();
            let completed = model
                .tasks
                .iter()
                .filter(|t| t.status == "completed")
                .count();
            let active_title = model
                .tasks
                .iter()
                .find(|t| t.status == "running" || t.status == "in_progress")
                .map(|t| t.title.as_str())
                .unwrap_or("executing");
            let task_idx = (completed + 1).min(total);

            row0.push(Span::raw(" · "));
            row0.push(Span::styled(
                format!("Task {task_idx}/{total} ({active_title})"),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        } else if !model.objective.is_empty() {
            row0.push(Span::raw(" | "));
            row0.push(Span::styled(&model.objective, tokens.text_secondary));
        }

        vec![Line::from(row0)]
    };

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(tokens.border_default);

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}
