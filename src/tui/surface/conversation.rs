//! Primary Conversational Execution Surface (Section 6, 7, 8, 21, 22).
//!
//! Delivers the primary stream of structured autonomous engineering activity:
//! - Progressive disclosure across compact, expanded, and detail states
//! - Bounded tool invocations with externalized artifact pointers
//! - Non-blocking viewport scrolling with auto-follow semantics
//! - Live agent execution spinner and intent telemetry
//! - Zero database or synchronous I/O during render

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the primary conversation & execution stream surface.
pub fn render_conversation_surface(
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

    // Split for live activity indicator if active
    let (history_area, activity_area) = if model.live_activity.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Governed lifecycle banner: the session always shows what the runtime is
    // waiting for, with exact revision identity when available.
    if model.lifecycle.stage != crate::tui::lifecycle::TuiLifecycleStage::Idle {
        let mut banner = format!("  {}", model.lifecycle.stage.label());
        if let Some(rev) = model.lifecycle.plan_revision {
            banner.push_str(&format!(" · plan rev {rev}"));
        }
        if let Some(hash) = model.lifecycle.plan_hash.as_ref() {
            let short: String = hash.chars().take(8).collect();
            banner.push_str(&format!(" ({short})"));
        }
        if let Some(rev) = model.lifecycle.task_revision {
            banner.push_str(&format!(" · tasks rev {rev}"));
        }
        if let Some(hash) = model.lifecycle.task_hash.as_ref() {
            let short: String = hash.chars().take(8).collect();
            banner.push_str(&format!(" ({short})"));
        }
        if let Some(auth) = model.lifecycle.authorization_id.as_ref() {
            banner.push_str(&format!(" · auth {auth}"));
        }
        lines.push(Line::styled(
            banner,
            if is_mono {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                tokens.status_warning.add_modifier(Modifier::BOLD)
            },
        ));
        lines.push(Line::raw(""));
    }

    if model.conversation.is_empty() {
        render_welcome_empty_state(&mut lines, tokens, is_mono);
    } else {
        let max_content_width = history_area.width.saturating_sub(4);
        for item in &model.conversation {
            let rendered = item.render_lines_with_icons(max_content_width, tokens, &model.icons);
            lines.extend(rendered);
            lines.push(Line::raw(""));
        }
    }

    // Viewport scrolling & auto-follow logic
    let total_lines = lines.len();
    let viewport_height = history_area.height.saturating_sub(2) as usize;

    let scroll_y = if total_lines > viewport_height {
        let max_scroll = total_lines.saturating_sub(viewport_height);
        max_scroll.saturating_sub(model.scroll_offset)
    } else {
        0
    };

    let scroll_status = if model.scroll_offset > 0 {
        format!("[Scrolled +{} · End to bottom]", model.scroll_offset)
    } else {
        "[Live Follow]".to_string()
    };

    let title = format!(
        " Conversation Timeline ({} items) {} ",
        model.conversation.len(),
        scroll_status
    );

    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let p = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, history_area);

    // Live Activity Spinner & Intent
    if let (Some(act_area), Some(activity_text)) = (activity_area, &model.live_activity) {
        let spin_symbol = model.spinner.current();

        let elapsed_str = if let Some(started_at) = model.activity_started_at {
            let secs = (chrono::Utc::now() - started_at).num_seconds();
            if secs > 0 {
                format!(" ({secs}s)")
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let live_tool_str = if let Some(running_tool) = model
            .live_tools
            .iter()
            .rev()
            .find(|t| t.state == crate::tui::model::LiveToolState::Running)
        {
            format!(" [tool: `{}`]", running_tool.tool_name)
        } else {
            String::new()
        };

        let act_line = Line::from(vec![
            Span::styled(
                format!(" {spin_symbol} Activity: "),
                if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    tokens.status_running.add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(
                format!("{activity_text}{live_tool_str}{elapsed_str}"),
                if is_mono {
                    Style::default()
                } else {
                    tokens.text_primary
                },
            ),
        ]);

        let act_block = Block::default()
            .borders(Borders::ALL)
            .border_style(if is_mono {
                Style::default()
            } else {
                tokens.status_running
            });

        let act_widget = Paragraph::new(act_line).block(act_block);
        f.render_widget(act_widget, act_area);
    }
}

fn render_welcome_empty_state(lines: &mut Vec<Line<'_>>, tokens: &ThemeTokens, is_mono: bool) {
    let welcome_style = if is_mono {
        Style::default()
    } else {
        tokens.text_muted
    };
    let accent_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        tokens.accent_primary.add_modifier(Modifier::BOLD)
    };
    let cmd_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  ◆ M31A Autonomous Software Engineering Agent",
        accent_style,
    ));
    lines.push(Line::styled(
        "    The model proposes. The runtime decides.",
        welcome_style,
    ));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Start by describing what you want M31A to build, fix, inspect, or explain.",
        tokens.text_primary,
    ));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("    /help    ", cmd_style),
        Span::styled("Show available commands and usage guide", welcome_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    /status  ", cmd_style),
        Span::styled(
            "Inspect current workspace, git state, and mission",
            welcome_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    /diff    ", cmd_style),
        Span::styled(
            "View unstaged modifications across the repository",
            welcome_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    /doctor  ", cmd_style),
        Span::styled(
            "Run 6-category system and environment diagnostics",
            welcome_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    @file    ", cmd_style),
        Span::styled(
            "Reference workspace files with real-time completion",
            welcome_style,
        ),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Universal Shortcuts: [Ctrl+P] Command Palette · [Enter] Send · [Shift+Enter] Multiline · [Esc] Unfocus",
        welcome_style,
    ));
}
