//! Quiet conversation-first session cockpit.
//!
//! Hierarchy: quiet header → conversation → composer. No telemetry strip,
//! no auto-split on historical records, no boxed dashboard.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::composer::TuiComposer;
use crate::tui::model::TuiViewModel;
use crate::tui::theme::ThemeTokens;

/// Render the interactive Session Cockpit screen.
pub fn render_session_cockpit(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    composer: &TuiComposer,
    is_composer_focused: bool,
    tokens: &ThemeTokens,
) {
    // Vertical breakdown: quiet header (2), conversation (min), composer (3)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(area);

    let status_area = chunks[0];
    let middle_area = chunks[1];
    let composer_area = chunks[2];

    render_status_strip(f, status_area, model, tokens);

    // Conversation-first: always full width (constrained for readability on
    // very wide terminals). Secondary detail lives in explicit
    // screens/inspectors, never auto-split here.
    let middle_area = if middle_area.width > 128 {
        let w = 120.min(middle_area.width - 8);
        Rect {
            x: middle_area.x + (middle_area.width - w) / 2,
            y: middle_area.y,
            width: w,
            height: middle_area.height,
        }
    } else {
        middle_area
    };
    render_timeline_section(f, middle_area, model, tokens);

    render_composer_section(
        f,
        composer_area,
        composer,
        is_composer_focused,
        tokens,
        model.is_semantically_active(),
    );
}

/// Quiet header: M31A · model · branch · state. No tokens/cost/ids.
fn render_status_strip(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let (marker, state_label) =
        if model.lifecycle.stage.is_governance_gate() || !model.approvals.is_empty() {
            ("○", "Waiting")
        } else if model.is_semantically_active() {
            ("•", "Working")
        } else if model.mission_status == "failed" {
            ("×", "Failed")
        } else {
            ("○", "Ready")
        };
    let model_name = if model.active_model.is_empty() || model.active_model == "none" {
        String::new()
    } else {
        let base = model
            .active_model
            .rsplit('/')
            .next()
            .unwrap_or(&model.active_model);
        base.strip_suffix("-instruct").unwrap_or(base).to_string()
    };
    let branch = if model.git_branch.is_empty() || model.git_branch == "N/A" {
        String::new()
    } else {
        model.git_branch.clone()
    };
    let mut spans = vec![Span::styled(" M31A ", tokens.text_primary)];
    if !model_name.is_empty() {
        spans.push(Span::styled(
            format!(" {model_name}"),
            tokens.text_secondary,
        ));
        spans.push(Span::styled(" · ", tokens.text_muted));
    } else {
        spans.push(Span::styled(" · ", tokens.text_muted));
    }
    if !branch.is_empty() {
        spans.push(Span::styled(branch, tokens.text_secondary));
        spans.push(Span::styled(" · ", tokens.text_muted));
    }
    spans.push(Span::styled(marker, tokens.text_secondary));
    spans.push(Span::styled(
        format!(" {state_label}"),
        tokens.text_secondary,
    ));
    // Semantic execution detail (task progress lives here once, not twice).
    if model.is_semantically_active() {
        if let Some(summary) = model.execution_summary() {
            let short = if summary.len() > 40 {
                format!("{}…", &summary[..40])
            } else {
                summary
            };
            spans.push(Span::styled(format!(" · {short}"), tokens.text_muted));
        } else if !model.tasks.is_empty() {
            let total = model.tasks.len();
            let completed = model
                .tasks
                .iter()
                .filter(|t| t.status == "completed")
                .count();
            spans.push(Span::styled(
                format!(" · Task {}/{} ", completed.min(total).max(1), total),
                tokens.text_muted,
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Open conversation stream with quiet inline execution state.
fn render_timeline_section(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::{Block, Paragraph, Wrap};

    let has_status = model.is_semantically_active() || !model.approvals.is_empty();
    let (status_area, history_area) = if has_status && area.height >= 6 {
        let sub = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(area);
        (Some(sub[0]), sub[1])
    } else {
        (None, area)
    };

    if let Some(sa) = status_area {
        let line = if !model.approvals.is_empty() {
            Line::from(Span::styled(
                " ○ Waiting for your approval",
                tokens.text_secondary,
            ))
        } else if let Some(s) = model.execution_summary() {
            Line::from(vec![
                Span::styled(format!(" {} ", model.spinner.current()), tokens.text_muted),
                Span::styled(s, tokens.text_secondary),
            ])
        } else {
            Line::from(Span::styled(" • Working", tokens.text_secondary))
        };
        f.render_widget(Paragraph::new(line), sa);
    }

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    if model.conversation.is_empty() {
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(Span::styled("  M31A", tokens.text_primary)));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(Span::styled(
            "  Your autonomous software engineering workspace.",
            tokens.text_secondary,
        )));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(Span::styled(
            "  Describe what you want to build, fix, inspect, or understand.",
            tokens.text_secondary,
        )));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(Span::styled("  > ", tokens.text_muted)));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(Span::styled(
            "  / for commands   @ for files   ? for help",
            tokens.text_muted,
        )));
    } else {
        for item in &model.conversation {
            let rendered = item.render_lines(history_area.width.saturating_sub(2), tokens);
            for l in rendered {
                all_lines.push(l);
            }
            all_lines.push(Line::raw(""));
        }
    }

    let total_lines = all_lines.len();
    let viewport_height = history_area.height as usize;

    let scroll_y = if total_lines > viewport_height {
        let max_scroll = total_lines.saturating_sub(viewport_height);
        max_scroll.saturating_sub(model.scroll_offset)
    } else {
        0
    };

    let p = Paragraph::new(all_lines)
        .block(Block::default())
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, history_area);
}

/// Quiet open composer.
fn render_composer_section(
    f: &mut Frame,
    area: Rect,
    composer: &TuiComposer,
    is_composer_focused: bool,
    tokens: &ThemeTokens,
    _is_busy: bool,
) {
    if is_composer_focused {
        composer.render(f, area, tokens);
    } else {
        let text = composer.text();
        let placeholder = if text.is_empty() {
            "Ask M31A to build, inspect, fix, or explain..."
        } else {
            text
        };
        let line = Line::from(vec![
            Span::styled("> ", tokens.text_muted),
            Span::styled(
                placeholder.to_string(),
                if text.is_empty() {
                    tokens.text_muted
                } else {
                    tokens.text_secondary
                },
            ),
        ]);
        // Hairline + prompt (no box).
        if area.height >= 2 {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(
                        " {}",
                        "─".repeat((area.width as usize).saturating_sub(2).min(120))
                    ),
                    tokens.separator,
                ))),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
            );
            f.render_widget(
                Paragraph::new(line),
                Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: area.height.saturating_sub(1),
                },
            );
        } else {
            f.render_widget(Paragraph::new(line), area);
        }
    }
}

/// Quiet overview retained for explicit inspector use (not auto-split).
#[allow(dead_code)]
fn render_status_overview(f: &mut Frame, area: Rect, model: &TuiViewModel, tokens: &ThemeTokens) {
    use ratatui::widgets::{Block, Borders, Paragraph};
    if area.height == 0 || area.width == 0 {
        return;
    }
    let completed = model
        .tasks
        .iter()
        .filter(|t| t.status == "completed")
        .count();
    let text = format!(
        "Overview\n\n{} tasks · {} complete · {} agents · {} approvals",
        model.tasks.len(),
        completed,
        model.agents.len(),
        model.approvals.len()
    );
    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(tokens.separator),
        ),
        area,
    );
}
