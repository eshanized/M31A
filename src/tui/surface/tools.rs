//! Tool & Capability Telemetry Surface (Section 8, TUI-01).
//!
//! Visualizes registered canonical tools, execution traces, error rates, risk tiers,
//! and invocation telemetry backed by ToolRegistry and runtime events.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::component::badge::render_risk_badge;
use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Consolidated tool item for presentation.
#[derive(Debug, Clone)]
pub struct DisplayToolItem {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub risk_tier: String,
    pub is_read_only: bool,
    pub executions_count: usize,
    pub errors_count: usize,
}

/// Render the Tools & Capabilities surface.
pub fn render_tools_surface(
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

    let mut items = Vec::new();
    if !model.canonical_tools.is_empty() {
        for ct in &model.canonical_tools {
            let tele = model.tools.iter().find(|t| t.name == ct.name);
            items.push(DisplayToolItem {
                name: ct.name.clone(),
                description: ct.description.clone(),
                capabilities: ct.capability_requirements.clone(),
                risk_tier: ct.risk_level.clone(),
                is_read_only: ct.is_read_only,
                executions_count: tele.map(|t| t.executions_count).unwrap_or(0),
                errors_count: tele.map(|t| t.errors_count).unwrap_or(0),
            });
        }
    } else {
        for t in &model.tools {
            let risk_tier = match t.name.as_str() {
                "fs_write" | "fs_delete" | "shell_exec" => "High",
                "git_commit" | "git_push" => "Critical",
                "exec_cargo_test" | "exec_doctor" => "Medium",
                _ => "Low",
            };
            items.push(DisplayToolItem {
                name: t.name.clone(),
                description: "Registered runtime tool".to_string(),
                capabilities: Vec::new(),
                risk_tier: risk_tier.to_string(),
                is_read_only: risk_tier == "Low",
                executions_count: t.executions_count,
                errors_count: t.errors_count,
            });
        }
    }

    let title = format!(" Tools & Capabilities ({} registered) ", items.len());

    if items.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  No tools registered or active in projection.",
                tokens.text_muted.add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "  Tools are registered through canonical ToolRegistry (TL-01).",
                tokens.text_secondary,
            ),
            Line::styled(
                "  All invocations pass through L1 Policy Precedence before execution.",
                tokens.text_muted,
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

    if area.width >= 70 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_tools_list(f, chunks[0], &items, tokens, selected_idx, border_style);
        render_tool_detail(f, chunks[1], &items, tokens, selected_idx, border_style);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_tools_list(f, chunks[0], &items, tokens, selected_idx, border_style);
        render_tool_detail(f, chunks[1], &items, tokens, selected_idx, border_style);
    }
}

fn render_tools_list(
    f: &mut Frame,
    area: Rect,
    tools: &[DisplayToolItem],
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let mut lines = Vec::new();
    let max_name_w = 18.min(area.width.saturating_sub(22) as usize);

    for (i, tool) in tools.iter().enumerate() {
        let is_selected = i == selected_idx.min(tools.len().saturating_sub(1));
        let error_style = if tool.errors_count > 0 {
            tokens.status_failed
        } else {
            tokens.status_ok
        };

        let row_style = if is_selected {
            Style::default()
                .bg(tokens.bg_surface.bg.unwrap_or(Color::DarkGray))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let prefix = if is_selected { "▸ " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled(prefix, tokens.accent_primary),
            Span::styled(format!("{:<w$} ", tool.name, w = max_name_w), row_style),
            Span::styled(
                format!("{:>4} calls ", tool.executions_count),
                tokens.text_secondary,
            ),
            Span::styled(format!("({} err)", tool.errors_count), error_style),
        ]));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(format!(" Tools ({}) ", tools.len()))
            .borders(Borders::NONE)
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn render_tool_detail(
    f: &mut Frame,
    area: Rect,
    tools: &[DisplayToolItem],
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let idx = selected_idx.min(tools.len().saturating_sub(1));
    let tool = &tools[idx];

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("Tool Name    : {}", tool.name),
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled("Description  : ", tokens.text_muted),
        Span::styled(&tool.description, tokens.text_primary),
    ]));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled("Invocations  : ", tokens.text_muted),
        Span::styled(
            format!("{}", tool.executions_count),
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ),
    ]));

    let err_style = if tool.errors_count > 0 {
        tokens.status_failed
    } else {
        tokens.status_ok
    };
    lines.push(Line::from(vec![
        Span::styled("Failures     : ", tokens.text_muted),
        Span::styled(format!("{}", tool.errors_count), err_style),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Risk Tier    : ", tokens.text_muted),
        render_risk_badge(&tool.risk_tier, tokens),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Side Effects : ", tokens.text_muted),
        Span::styled(
            if tool.is_read_only {
                "ReadOnly (Safe)"
            } else {
                "Mutating (Governed)"
            },
            if tool.is_read_only {
                tokens.status_ok
            } else {
                tokens.status_warning
            },
        ),
    ]));

    let caps_str = if tool.capabilities.is_empty() {
        "Default / Unrestricted".to_string()
    } else {
        tool.capabilities.join(", ")
    };
    lines.push(Line::from(vec![
        Span::styled("Capabilities : ", tokens.text_muted),
        Span::styled(caps_str, tokens.accent_secondary),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled("Policy Enforcement (L1):", tokens.text_muted));
    lines.push(Line::styled(
        "  • Sandboxing: isolated workspace paths only",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Approval Gate: requires operator consent on High/Critical",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Replay Safety: pure read-only during replay mode",
        tokens.text_secondary,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Tool [{}] ", tool.name))
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
