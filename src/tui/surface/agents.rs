//! Agent Swarm & Autonomous Roles Surface (Section 14, TUI-01).
//!
//! Visualizes autonomous agent swarm state, active tasks, token consumption,
//! model routing, and operational status badges.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Agent Swarm surface.
pub fn render_agents_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_focused: bool,
    selected_idx: usize,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    let title = format!(
        " Agent Swarm & Roles ({} active) ",
        crate::tui::model::count_active_agents(&model.agents)
    );

    if model.agents.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  No autonomous agents currently spawned.",
                tokens.text_muted,
            ),
            Line::styled(
                "  Agents are dynamically spawned by the scheduler according to DAG roles.",
                tokens.text_secondary,
            ),
        ])
        .block(
            Block::default()
                .title(title)
                .borders(Borders::NONE)
                .border_style(border_style),
        );
        f.render_widget(p, area);
        return;
    }

    if area.width >= 75 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_agents_table(f, chunks[0], model, tokens, selected_idx, border_style);
        render_agent_detail(f, chunks[1], model, tokens, selected_idx, border_style);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_agents_table(f, chunks[0], model, tokens, selected_idx, border_style);
        render_agent_detail(f, chunks[1], model, tokens, selected_idx, border_style);
    }
}

/// Canonical display presentation for agent lifecycle states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStateDisplay {
    pub symbol: &'static str,
    pub label: String,
    pub style: Style,
    pub is_unknown: bool,
}

/// Shared semantic status mapping for the agent list and inspector (REM-09).
pub fn map_agent_state(state: &str, tokens: &ThemeTokens) -> AgentStateDisplay {
    let norm = state.trim().to_ascii_lowercase();
    match norm.as_str() {
        "executing" | "running" => AgentStateDisplay {
            symbol: "●",
            label: "Working".to_string(),
            style: tokens.status_running,
            is_unknown: false,
        },
        "completed" | "succeeded" | "success" => AgentStateDisplay {
            symbol: "✓",
            label: "Completed".to_string(),
            style: tokens.status_ok,
            is_unknown: false,
        },
        "failed" | "error" => AgentStateDisplay {
            symbol: "✗",
            label: "Failed".to_string(),
            style: tokens.status_failed,
            is_unknown: false,
        },
        "cancelled" | "canceled" => AgentStateDisplay {
            symbol: "⊘",
            label: "Cancelled".to_string(),
            style: tokens.status_failed,
            is_unknown: false,
        },
        "blocked" => AgentStateDisplay {
            symbol: "⛔",
            label: "Blocked".to_string(),
            style: tokens.status_blocked,
            is_unknown: false,
        },
        "waiting" | "idle" | "awaiting" => AgentStateDisplay {
            symbol: "○",
            label: "Waiting".to_string(),
            style: tokens.status_waiting,
            is_unknown: false,
        },
        _ => {
            let raw = state.trim();
            let label = if raw.is_empty() {
                "Unknown".to_string()
            } else {
                format!("Unknown ({raw})")
            };
            AgentStateDisplay {
                symbol: "?",
                label,
                style: tokens.text_muted,
                is_unknown: true,
            }
        }
    }
}

fn render_agents_table(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let mut lines = Vec::new();
    let max_role_w = 18.min(area.width.saturating_sub(20) as usize);

    let viewport_height = (area.height as usize).saturating_sub(1).max(1);
    let (start_idx, end_idx) =
        crate::tui::surface::visible_list_range(model.agents.len(), selected_idx, viewport_height);

    for (local_i, agent) in model.agents[start_idx..end_idx].iter().enumerate() {
        let actual_idx = start_idx + local_i;
        let is_selected = actual_idx == selected_idx.min(model.agents.len().saturating_sub(1));
        let state_disp = map_agent_state(&agent.state, tokens);
        let sym = state_disp.symbol;
        let sym_style = state_disp.style;

        let truncated_role = if agent.role.len() > max_role_w {
            format!("{:.w$}...", agent.role, w = max_role_w.saturating_sub(3))
        } else {
            agent.role.clone()
        };

        let row_style = if is_selected {
            Style::default()
                .bg(tokens.bg_surface.bg.unwrap_or(Color::DarkGray))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let prefix = if is_selected { "▸ " } else { "  " };

        let tok_str = if agent.total_tokens >= 1000 {
            format!("{:.1}k tok", agent.total_tokens as f64 / 1000.0)
        } else {
            format!("{} tok", agent.total_tokens)
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, tokens.accent_primary),
            Span::styled(format!("{sym} "), sym_style),
            Span::styled(
                format!("{:<w$} ", truncated_role, w = max_role_w),
                row_style,
            ),
            Span::styled(format!("{:>9}", tok_str), tokens.text_muted),
        ]));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(" Agents ")
            .borders(Borders::NONE)
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn render_agent_detail(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let idx = selected_idx.min(model.agents.len().saturating_sub(1));
    let agent = &model.agents[idx];

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("Agent: {}", agent.id),
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::styled(
        format!("Role: {}", agent.role),
        tokens.text_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    let state_disp = map_agent_state(&agent.state, tokens);
    let mut status_spans = vec![Span::styled("State        ", tokens.text_muted)];
    status_spans.push(Span::styled(
        format!("{} ", state_disp.symbol),
        state_disp.style,
    ));
    status_spans.push(Span::styled(state_disp.label, state_disp.style));
    lines.push(Line::from(status_spans));

    let task_str = agent
        .current_task
        .as_deref()
        .unwrap_or("idle / awaiting dispatch");
    lines.push(Line::from(vec![
        Span::styled("Current Task ", tokens.text_muted),
        Span::styled(task_str, tokens.text_secondary),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Tokens Used  ", tokens.text_muted),
        Span::styled(
            format!("{} tokens", agent.total_tokens),
            tokens.text_secondary,
        ),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Capabilities & Sandboxing:",
        tokens.text_muted,
    ));
    lines.push(Line::styled(
        "  • Sandboxed filesystem access (workspace relative)",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Process manager authority (supervised child proc)",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Strict policy interception on mutating side-effects",
        tokens.text_secondary,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Agent [{}] ", agent.role))
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
