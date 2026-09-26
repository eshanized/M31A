//! Semantic Status Badges and Visual Chips (TDS-04, D-03).
//!
//! Provides non-color dependent semantic badges, symbols, and role tags
//! for high-clarity terminal presentation.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::tui::status::StatusKind;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render a status badge with icon and bracketed tag.
pub fn render_status_badge<'a>(status: StatusKind, tokens: &ThemeTokens) -> Vec<Span<'a>> {
    let (badge_str, style) = crate::tui::status::StatusPresentation::format(status, tokens);
    let icon = match status {
        StatusKind::Ok => "✓ ",
        StatusKind::Running => "● ",
        StatusKind::Waiting => "○ ",
        StatusKind::Blocked => "⛔ ",
        StatusKind::Failed => "✗ ",
        StatusKind::Warning => "⚠ ",
        StatusKind::Asking => "❓ ",
        StatusKind::Denied => "⊘ ",
        StatusKind::Planning => "◆ ",
        StatusKind::Verifying => "🔍 ",
        StatusKind::Paused => "⏸ ",
        StatusKind::Cancelled => "✕ ",
    };

    vec![Span::styled(icon, style), Span::styled(badge_str, style)]
}

/// Render a risk tier badge: Low, Medium, High, Critical.
pub fn render_risk_badge<'a>(risk: &str, tokens: &ThemeTokens) -> Span<'a> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let lower = risk.to_lowercase();
    let (text, style) = match lower.as_str() {
        "critical" | "crit" => {
            let style = if is_mono {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                tokens.status_failed.add_modifier(Modifier::BOLD)
            };
            ("[RISK:CRIT]", style)
        }
        "high" => {
            let style = if is_mono {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                tokens.status_warning.add_modifier(Modifier::BOLD)
            };
            ("[RISK:HIGH]", style)
        }
        "medium" | "med" => {
            let style = if is_mono {
                Style::default()
            } else {
                tokens.status_waiting
            };
            ("[RISK:MED]", style)
        }
        _ => {
            let style = if is_mono {
                Style::default()
            } else {
                tokens.text_muted
            };
            ("[RISK:LOW]", style)
        }
    };

    Span::styled(text, style)
}

/// Render an agent role tag (e.g. `[LeadOrchestrator]`).
pub fn render_agent_tag<'a>(role: &str, tokens: &ThemeTokens) -> Span<'a> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        tokens.accent_primary
    };
    Span::styled(format!("◆ {role}"), style)
}

/// Render a keyboard hint chip (e.g. `[Enter]`).
pub fn render_key_chip<'a>(key: &str, tokens: &ThemeTokens) -> Span<'a> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };
    Span::styled(format!("[{key}]"), style)
}
