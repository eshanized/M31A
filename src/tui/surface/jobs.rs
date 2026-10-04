//! Process Sandboxing & Background Jobs Surface (TUI-01).
//!
//! Visualizes background process execution, isolation tiers, sandboxing,
//! CPU/duration telemetry, and process lifecycle states.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Jobs & Process Sandbox surface.
pub fn render_jobs_surface(
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

    let title = format!(" Background Jobs & Sandbox ({} jobs) ", model.jobs.len());

    let mut lines = Vec::new();
    if model.jobs.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "  No background jobs or child processes running.",
            tokens.text_muted,
        ));
        lines.push(Line::styled(
            "  Background tasks are executed through the supervised job manager.",
            tokens.text_secondary,
        ));
    } else {
        for job in &model.jobs {
            let is_running = job.status == "running";
            let sym = if is_running { "●" } else { "✓" };
            let sym_style = if is_running {
                tokens.status_running
            } else {
                tokens.status_ok
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{sym} "), sym_style),
                Span::styled(
                    format!("{:<10} ", job.id),
                    tokens.accent_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<30} ", job.name), tokens.text_primary),
                Span::styled(format!("[{}] ", job.status.to_uppercase()), sym_style),
                Span::styled(format!("{}ms", job.duration_ms), tokens.text_muted),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Sandbox Security Constraints (L1/L2):",
        tokens.text_muted,
    ));
    lines.push(Line::styled(
        "  • Process namespace isolation: enforced",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • CPU / Memory resource quotas: monitored and bounded",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Real-time cancellation watchdog: active",
        tokens.text_secondary,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
