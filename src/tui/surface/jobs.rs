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

    let available_rows = (area.height as usize).saturating_sub(6).max(1);
    let (start_idx, end_idx) =
        crate::tui::surface::visible_list_range(model.jobs.len(), selected_idx, available_rows);

    let title = if model.jobs.len() > available_rows {
        format!(
            " Background Jobs & Sandbox (showing {}-{} of {}) ",
            start_idx + 1,
            end_idx,
            model.jobs.len()
        )
    } else {
        format!(" Background Jobs & Sandbox ({} jobs) ", model.jobs.len())
    };

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
        for (local_i, job) in model.jobs[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + local_i;
            let is_selected = actual_idx == selected_idx.min(model.jobs.len().saturating_sub(1));
            let norm_status = job.status.to_ascii_lowercase();
            let (sym, sym_style) = match norm_status.as_str() {
                "running" | "in_progress" => ("●", tokens.status_running),
                "completed" | "succeeded" | "success" => ("✓", tokens.status_ok),
                "failed" | "error" => ("✗", tokens.status_failed),
                "cancelled" | "canceled" => ("⊘", tokens.status_failed),
                "blocked" => ("⛔", tokens.status_blocked),
                "queued" | "pending" => ("○", tokens.status_waiting),
                _ => ("?", tokens.text_muted),
            };

            let row_style = if is_selected {
                Style::default()
                    .bg(tokens
                        .bg_surface
                        .bg
                        .unwrap_or(ratatui::style::Color::DarkGray))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if is_selected { "▸ " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(prefix, tokens.accent_primary),
                Span::styled(format!("{sym} "), sym_style),
                Span::styled(
                    format!("{:<10} ", job.id),
                    row_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<30} ", job.name), row_style),
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

    if let Some(ref sb) = model.sandbox_status {
        let (iso_val, iso_style) = if sb.isolation_active {
            ("enforced (active)", tokens.status_ok)
        } else if sb.mode.contains("disabled") || sb.mode.starts_with("unsupported") {
            ("disabled / not active", tokens.status_warning)
        } else if sb.mode.contains("uncontained") {
            ("unavailable / uncontained", tokens.status_warning)
        } else if sb.mode.contains("degraded") {
            ("degraded / inactive", tokens.status_warning)
        } else {
            ("capable (idle - no running jobs)", tokens.text_secondary)
        };
        lines.push(Line::from(vec![
            Span::styled("  • Process namespace isolation: ", tokens.text_secondary),
            Span::styled(iso_val, iso_style),
        ]));

        let (quota_val, quota_style) = if sb.quotas_enforced {
            ("monitored and bounded", tokens.status_ok)
        } else if sb.mode.contains("disabled") || sb.mode.starts_with("unsupported") {
            ("unverified / unconstrained", tokens.text_muted)
        } else if sb.mode.contains("uncontained") {
            ("unconstrained (uncontained mode)", tokens.text_muted)
        } else {
            ("configured (idle - no running jobs)", tokens.text_secondary)
        };
        lines.push(Line::from(vec![
            Span::styled("  • CPU / Memory resource quotas: ", tokens.text_secondary),
            Span::styled(quota_val, quota_style),
        ]));

        let (watchdog_val, watchdog_style) = if sb.watchdog_active {
            ("active", tokens.status_ok)
        } else if sb.mode.contains("disabled") || sb.mode.starts_with("unsupported") {
            ("inactive / not verified", tokens.text_muted)
        } else {
            ("ready (idle - no running jobs)", tokens.text_secondary)
        };
        lines.push(Line::from(vec![
            Span::styled(
                "  • Real-time cancellation watchdog: ",
                tokens.text_secondary,
            ),
            Span::styled(watchdog_val, watchdog_style),
        ]));

        let (net_val, net_style) = if sb.network_restricted {
            ("restricted (isolated)", tokens.status_ok)
        } else if sb.mode.contains("disabled") {
            ("unrestricted (disabled)", tokens.status_warning)
        } else if sb.mode.contains("uncontained") {
            ("unrestricted (host network shared)", tokens.text_muted)
        } else {
            ("configured (idle - no running jobs)", tokens.text_secondary)
        };
        lines.push(Line::from(vec![
            Span::styled("  • Network access restrictions: ", tokens.text_secondary),
            Span::styled(net_val, net_style),
        ]));
    } else {
        // Truthful reporting: no runtime sandbox evidence has been reported yet.
        lines.push(Line::styled(
            "  • Process namespace isolation: unverified (runtime evidence not reported)",
            tokens.text_muted,
        ));
        lines.push(Line::styled(
            "  • CPU / Memory resource quotas: unverified",
            tokens.text_muted,
        ));
        lines.push(Line::styled(
            "  • Real-time cancellation watchdog: not reported",
            tokens.text_muted,
        ));
        lines.push(Line::styled(
            "  • Network access restrictions: unverified",
            tokens.text_muted,
        ));
    }

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
