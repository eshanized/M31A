//! Telemetry & Model Usage Surface (Section 23, TUI-01).
//!
//! Provides comprehensive observability over runtime operations:
//! - Token consumption (prompt, completion, total) and estimated financial cost
//! - Uptime, processed event counts, RSS memory usage
//! - Active provider, model tier, and execution profile

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Model Usage & Telemetry surface.
pub fn render_telemetry_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_focused: bool,
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

    let total_tokens = model.model_usage.prompt_tokens + model.model_usage.completion_tokens;
    let cost_str = match model.model_usage.total_cost_cents {
        Some(cents) if cents > 0 => format!("${:.2} USD", cents as f64 / 100.0),
        Some(_) if total_tokens == 0 => "$0.00 USD".to_string(),
        Some(_) => "unpriced".to_string(),
        None if total_tokens > 0 => "unpriced".to_string(),
        None => "unknown".to_string(),
    };

    let mut lines = Vec::new();
    lines.push(Line::styled(
        "MODEL INFERENCE TELEMETRY",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    if let Some(status) = model.usage_section_state.status_line("Usage telemetry") {
        lines.push(Line::styled(status, tokens.status_warning));
    }
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled("Active Model     : ", tokens.text_muted),
        Span::styled(
            &model.active_model,
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Provider         : ", tokens.text_muted),
        Span::styled(&model.active_provider, tokens.text_secondary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Profile          : ", tokens.text_muted),
        Span::styled(&model.active_profile, tokens.accent_secondary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Prompt Tokens    : ", tokens.text_muted),
        Span::styled(
            format!("{}", model.model_usage.prompt_tokens),
            tokens.text_primary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Completion Tokens: ", tokens.text_muted),
        Span::styled(
            format!("{}", model.model_usage.completion_tokens),
            tokens.text_primary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Total Tokens     : ", tokens.text_muted),
        Span::styled(
            format!("{total_tokens}"),
            tokens.status_running.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Cumulative Cost  : ", tokens.text_muted),
        Span::styled(cost_str, tokens.status_ok.add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Inference Calls  : ", tokens.text_muted),
        Span::styled(
            format!("{}", model.model_usage.api_calls),
            tokens.text_primary,
        ),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "RUNTIME RESOURCE METRICS",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled("Uptime           : ", tokens.text_muted),
        Span::styled(
            format!("{} seconds", model.system_stats.uptime_secs),
            tokens.text_primary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Events Processed : ", tokens.text_muted),
        Span::styled(
            format!("{}", model.system_stats.events_processed),
            tokens.text_primary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Resident Memory  : ", tokens.text_muted),
        Span::styled(
            format!("{} MB RSS", model.system_stats.memory_rss_mb),
            tokens.text_primary,
        ),
    ]));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Model usage ")
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((model.detail_scroll_offset.min(u16::MAX as usize) as u16, 0));
    f.render_widget(p, area);
}
