//! Quiet context-sensitive shell footer.
//!
//! A single muted hint line. Focus is communicated by cursor placement and
//! subtle selection — never by large `[COMPOSER] / [STREAM] / [MODAL]` badges.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::focus::FocusTarget;
use crate::tui::lifecycle::TuiLifecycleStage;
use crate::tui::navigation::ScreenId;
use crate::tui::theme::ThemeTokens;

/// Render the quiet shell footer.
pub fn render_footer(
    f: &mut Frame,
    area: Rect,
    screen: ScreenId,
    focus: FocusTarget,
    is_composer_focused: bool,
    lifecycle: &crate::tui::lifecycle::TuiLifecycleProjection,
    tokens: &ThemeTokens,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let hint = footer_hint(screen, focus, is_composer_focused, lifecycle);
    let line = Line::from(Span::styled(format!(" {hint}"), tokens.text_muted));
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(tokens.separator);
    f.render_widget(Paragraph::new(vec![line]).block(block), area);
}

fn footer_hint(
    screen: ScreenId,
    focus: FocusTarget,
    is_composer_focused: bool,
    lifecycle: &crate::tui::lifecycle::TuiLifecycleProjection,
) -> String {
    if matches!(focus, FocusTarget::Overlay) {
        return "Esc back · Enter select · ↑↓ navigate".to_string();
    }
    if is_composer_focused {
        match lifecycle.stage {
            TuiLifecycleStage::PlanReviewRequired | TuiLifecycleStage::PlanRevisionAvailable => {
                return "Type to discuss · /plan accept · /plan revise · Esc back".to_string();
            }
            TuiLifecycleStage::TasksReviewRequired | TuiLifecycleStage::TaskRevisionAvailable => {
                return "Type to discuss · /tasks accept · /tasks regen · Esc back".to_string();
            }
            TuiLifecycleStage::ExecutionAuthorizationRequired => {
                return "Type to discuss · /authorize yes · /authorize no · Esc back".to_string();
            }
            TuiLifecycleStage::DiscoveryRequired => {
                return "Answer above · Enter send · Shift+Enter newline · Esc back".to_string();
            }
            _ => {}
        }
        return "Enter send · Shift+Enter newline · @ files · / commands".to_string();
    }
    match screen {
        ScreenId::Dashboard => "Type to write · Ctrl+P commands · ? help · 1-0 screens".to_string(),
        _ => "Esc home · Ctrl+P commands · ? help".to_string(),
    }
}
