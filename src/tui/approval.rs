//! Interactive Non-Blocking Policy Approval Modal (TUI-01, D-17).
//!
//! Renders high-visibility prompt with tool details, effective risk tier,
//! justification, parameter preview, and immediate resolution keybindings:
//! - `y`: Approve Once
//! - `a`: Approve Always for current mission
//! - `n`: Reject invocation
//! - `e`: Edit parameters
//! - `Esc`: Dismiss modal without deciding
//!
//! Non-blocking: background tasks in other DAG branches continue executing
//! concurrently while an approval modal is open.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use serde::{Deserialize, Serialize};

use super::model::TuiApprovalRequest;
use super::sanitizer::sanitize_terminal_text;

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

    /// Render floating modal dialog on frame if open.
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let Some(ref req) = self.current_request else {
            return;
        };

        if !self.is_open {
            return;
        }

        let modal_area = centered_approval_rect(70, 60, area);
        f.render_widget(Clear, modal_area);

        let risk_color = match req.risk_tier.to_lowercase().as_str() {
            "critical" | "high" => Color::Red,
            "medium" => Color::Yellow,
            _ => Color::Green,
        };

        let safe_justification = sanitize_terminal_text(&req.justification);
        let safe_params = sanitize_terminal_text(&req.parameters_summary);

        let body = format!(
            "POLICY AUTHORIZATION REQUIRED (D-17)\n\
             ====================================\n\n\
             Tool:          {}\n\
             Calling Agent: {}\n\
             Risk Level:    {}\n\
             Requested At:  {}\n\n\
             Justification:\n  {}\n\n\
             Parameters Preview:\n  {}\n\n\
             ------------------------------------\n\
             [y / Enter] Approve Once  |  [a] Approve Always (Mission)\n\
             [n] Reject                |  [e] Edit Parameters\n\
             [Esc] Dismiss Dialog (Defer)",
            req.tool_name,
            req.agent_role,
            req.risk_tier.to_uppercase(),
            req.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            safe_justification,
            safe_params
        );

        let block = Paragraph::new(body)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Policy Approval Intercept ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(risk_color).add_modifier(Modifier::BOLD)),
            );

        f.render_widget(block, modal_area);
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
