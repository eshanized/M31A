//! Quiet semantic status badges.
//!
//! Color supports meaning; symbol + text carries it. No emoji blocks.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::tui::status::StatusKind;
use crate::tui::theme::ThemeTokens;

/// Render a status badge with quiet symbol and plain label.
pub fn render_status_badge<'a>(status: StatusKind, tokens: &ThemeTokens) -> Vec<Span<'a>> {
    let (_, style) = crate::tui::status::StatusPresentation::format(status, tokens);
    let symbol = match status {
        StatusKind::Ok => "✓ ",
        StatusKind::Running => "• ",
        StatusKind::Waiting => "○ ",
        StatusKind::Blocked => "! ",
        StatusKind::Failed => "× ",
        StatusKind::Warning => "! ",
        StatusKind::Asking => "? ",
        StatusKind::Denied => "× ",
        StatusKind::Planning => "• ",
        StatusKind::Verifying => "• ",
        StatusKind::Paused => "○ ",
        StatusKind::Cancelled => "× ",
    };
    let label = match status {
        StatusKind::Ok => "Completed",
        StatusKind::Running => "Working",
        StatusKind::Waiting => "Waiting",
        StatusKind::Blocked => "Blocked",
        StatusKind::Failed => "Failed",
        StatusKind::Warning => "Needs attention",
        StatusKind::Asking => "Needs input",
        StatusKind::Denied => "Denied",
        StatusKind::Planning => "Planning",
        StatusKind::Verifying => "Verifying",
        StatusKind::Paused => "Paused",
        StatusKind::Cancelled => "Cancelled",
    };

    vec![Span::styled(symbol, style), Span::styled(label, style)]
}

/// Render a risk tier badge (quiet text, no shouting).
pub fn render_risk_badge<'a>(risk: &str, tokens: &ThemeTokens) -> Span<'a> {
    let lower = risk.to_lowercase();
    let (text, style) = match lower.as_str() {
        "critical" | "crit" => ("Risk: critical", tokens.error),
        "high" => ("Risk: high", tokens.warning),
        "medium" | "med" => ("Risk: medium", tokens.text_secondary),
        _ => ("Risk: low", tokens.text_muted),
    };

    Span::styled(text, style)
}

/// Render an agent role tag (quiet).
pub fn render_agent_tag<'a>(role: &str, tokens: &ThemeTokens) -> Span<'a> {
    Span::styled(role.to_string(), tokens.text_secondary)
}

/// Render a keyboard hint chip (quiet, no yellow).
pub fn render_key_chip<'a>(key: &str, tokens: &ThemeTokens) -> Span<'a> {
    Span::styled(key.to_string(), tokens.text_secondary)
}

/// Compatibility helper retained for footer hints.
pub fn format_key_hints_placeholder() -> Style {
    Style::default()
}
