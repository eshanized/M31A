//! Doctor Diagnostics Surface (TUI-01, CLI-04).
//!
//! Visualizes 6-category system health, environment prerequisites,
//! and runtime configuration diagnostics.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::{DoctorStatus, TuiViewModel};
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Doctor Diagnostics surface.
pub fn render_doctor_surface(
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

    let title = " Doctor System Diagnostics [0] ";

    let mut lines = Vec::new();
    lines.push(Line::styled(
        "System Health Diagnostics:",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    if model.doctor_checks.is_empty() {
        let unexecuted_probes = [
            (
                "Environment & Toolchain",
                "rustc, cargo, git toolchain prerequisites (not evaluated)",
            ),
            (
                "Workspace Integrity",
                "Valid Cargo.toml and git repository anchor (not evaluated)",
            ),
            (
                "Persistence & SQLite",
                "WAL mode, pool connectivity, zero lock contention (not evaluated)",
            ),
            (
                "Model Provider Credentials",
                "Active inference provider credentials (not evaluated)",
            ),
            (
                "Policy & Sandbox Kernel",
                "Fail-closed capability gates and policy rules (not evaluated)",
            ),
            (
                "Verification Test Harness",
                "Local test harness and cargo execution pipeline (not evaluated)",
            ),
        ];

        for (cat, detail) in unexecuted_probes {
            lines.push(Line::from(vec![
                Span::styled("[NOT RUN] ", tokens.text_muted),
                Span::styled(
                    format!("{:<30} ", cat),
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(detail, tokens.text_secondary),
            ]));
        }
    } else {
        for check in &model.doctor_checks {
            let (icon, sym_style) = match check.status {
                DoctorStatus::Pass => ("[PASS] ", tokens.status_ok),
                DoctorStatus::Fail => ("[FAIL] ", tokens.status_failed),
                DoctorStatus::Warning => ("[WARN] ", tokens.status_warning),
                DoctorStatus::Unverified => ("[UNVERIFIED] ", tokens.text_muted),
                DoctorStatus::Unavailable => ("[UNAVAILABLE] ", tokens.status_failed),
                DoctorStatus::NotRun => ("[NOT RUN] ", tokens.text_muted),
            };

            let ts_str = check
                .evaluated_at
                .map(|t| format!(" [{}]", t.format("%H:%M:%S")))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(icon, sym_style),
                Span::styled(
                    format!("{:<20} {:<15} ", check.category, check.name),
                    tokens.text_primary.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{}{ts_str}", check.detail), tokens.text_secondary),
            ]));

            if let Some(ref rem) = check.remediation {
                lines.push(Line::from(vec![
                    Span::raw("       "),
                    Span::styled(format!("↳ Remediation: {rem}"), tokens.text_muted),
                ]));
            }
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled("Diagnostic Invariants:", tokens.text_muted));
    lines.push(Line::styled(
        "  • Diagnostic checks run without side-effects or persistent state mutation.",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • Run `/doctor` from composer to execute full diagnostic probe suite.",
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
