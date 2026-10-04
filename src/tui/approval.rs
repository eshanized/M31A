//! Interactive policy approval modal.
//!
//! High importance — clearly elevated — but professional, not cyberpunk.
//! Keys: y/Enter approve once, a approve always, n reject, e edit,
//! Esc dismiss. Behavior preserved; only presentation rebuilt.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use serde::{Deserialize, Serialize};

use super::model::TuiApprovalRequest;
use super::sanitizer::sanitize_terminal_text;
use super::theme::ThemeTokens;

/// Operator decision responding to a policy approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    ApproveOnce,
    ApproveAlways,
    Reject,
    Edit,
    Dismiss,
}

/// Controller and renderer for the interactive approval modal.
#[derive(Debug, Clone, Default)]
pub struct ApprovalModal {
    pub current_request: Option<TuiApprovalRequest>,
    pub is_open: bool,
}

impl ApprovalModal {
    pub fn new() -> Self {
        Self {
            current_request: None,
            is_open: false,
        }
    }

    pub fn open(&mut self, request: TuiApprovalRequest) {
        self.current_request = Some(request);
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.current_request = None;
        self.is_open = false;
    }

    /// Process keyboard input on the active approval modal.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ApprovalDecision> {
        if !self.is_open || self.current_request.is_none() {
            return None;
        }

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.close();
                Some(ApprovalDecision::ApproveOnce)
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.close();
                Some(ApprovalDecision::ApproveAlways)
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.close();
                Some(ApprovalDecision::Reject)
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.close();
                Some(ApprovalDecision::Edit)
            }
            KeyCode::Esc => {
                self.close();
                Some(ApprovalDecision::Dismiss)
            }
            _ => None,
        }
    }

    /// Render floating modal dialog on frame if open (theme-aware).
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let tokens = ThemeTokens::resolve(crate::tui::theme::ThemeMode::Default);
        self.render_with_theme(f, area, &tokens);
    }

    /// Render with explicit theme tokens.
    pub fn render_with_theme(&self, f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
        let Some(ref req) = self.current_request else {
            return;
        };

        if !self.is_open {
            return;
        }

        let modal_area = centered_approval_rect(66, 60, area);
        f.render_widget(Clear, modal_area);

        let safe_justification = sanitize_terminal_text(&req.justification);
        let safe_params = sanitize_terminal_text(&req.parameters_summary);
        let risk_label = req.risk_tier.to_lowercase();
        let risk_style = match risk_label.as_str() {
            "critical" | "high" => tokens.error,
            "medium" => tokens.warning,
            _ => tokens.text_muted,
        };

        let mut lines: Vec<Line> = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "  M31A needs your approval",
                tokens.text_primary,
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("  Command  ", tokens.text_muted),
                Span::styled(req.tool_name.clone(), tokens.text_primary),
            ]),
            Line::from(vec![
                Span::styled("  Details  ", tokens.text_muted),
                Span::styled(truncate(&safe_params, 120), tokens.text_secondary),
            ]),
            Line::raw(""),
            Line::from(Span::styled("  Reason", tokens.text_muted)),
            Line::from(Span::styled(
                format!("  {}", truncate(&safe_justification, 240)),
                tokens.text_secondary,
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("  Risk  ", tokens.text_muted),
                Span::styled(req.risk_tier.clone(), risk_style),
                Span::styled(
                    format!("  ·  {}", req.timestamp.format("%H:%M:%S")),
                    tokens.text_muted,
                ),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                "  ────────────────────────────────",
                tokens.separator,
            )),
            Line::from(Span::styled(
                "  [Approve Once]  [Approve Always]  [Reject]  [Edit]",
                tokens.text_secondary,
            )),
            Line::from(Span::styled(
                "  y / Enter       a                 n         e      ·  Esc dismiss",
                tokens.text_muted,
            )),
        ];
        let _ = Style::default();
        // Clamp to modal height
        let max = modal_area.height.saturating_sub(2) as usize;
        lines.truncate(max.max(1));

        let block = Block::default()
            .title(" Approval ")
            .borders(Borders::ALL)
            .border_style(tokens.separator);

        f.render_widget(Paragraph::new(lines).block(block), modal_area);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 1 {
        String::new()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn centered_approval_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
