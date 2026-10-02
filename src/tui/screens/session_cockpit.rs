//! Interactive Session Cockpit Screen Renderer (COP-01, CLI-01, CLI-02, CLI-04).
//!
//! Delivers the primary agentic coding cockpit:
//! - Top status strip (Session ID, Model, Provider, Profile, Git branch, Mission status)
//! - Live Conversation Timeline with semantic badges (`[USER]`, `[ASST]`, `[TOOL]`, `[VERIFY]`, `[ASK]`)
//! - Non-blocking viewport scrolling (scroll offset from ViewModel)
//! - Live Agent Activity & Tool Execution Indicator
//! - Bottom Interactive Composer with autocomplete popup and modal focus states

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::composer::TuiComposer;
use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the interactive Session Cockpit screen.
pub fn render_session_cockpit(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    composer: &TuiComposer,
    is_composer_focused: bool,
    tokens: &ThemeTokens,
) {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || std::env::var("NO_COLOR").is_ok();

    // Vertical breakdown: Header Status (3), Main Workspace (Min 6), Composer (4)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(4),
        ])
        .split(area);

    let status_area = chunks[0];
    let middle_area = chunks[1];
    let composer_area = chunks[2];

    // 1. Render Status Strip
    render_status_strip(f, status_area, model, is_mono);

    // 2. Render Middle Workspace: Split-pane when active tasks exist, full-width timeline when idle (COP-01)
    let has_active_work =
        !model.tasks.is_empty() || !model.agents.is_empty() || !model.approvals.is_empty();
    if has_active_work {
        let is_wide = area.width >= 90;
        let (timeline_area, overview_area) = if is_wide {
            let (left_pct, right_pct) = if area.width >= 110 {
                (65, 35)
            } else {
                (60, 40)
            };
            let middle_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(left_pct),
                    Constraint::Percentage(right_pct),
                ])
                .split(middle_area);
            (middle_cols[0], middle_cols[1])
        } else {
            let middle_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(middle_area);
            (middle_rows[0], middle_rows[1])
        };

        // Render Conversation Timeline & Live Activity
        render_timeline_section(f, timeline_area, model, tokens, is_mono);

        // Render Status Overview, Tasks, and Recent Logs
        render_status_overview(f, overview_area, model, is_mono);
    } else {
        // Conversation-First: Full width dedicated to Conversation Timeline & Live Activity
        render_timeline_section(f, middle_area, model, tokens, is_mono);
    }

    // 3. Render Bottom Composer
    let is_busy = model.mission_status == "running"
        || model.session_status == "running"
        || model.mission_status == "executing";
    render_composer_section(
        f,
        composer_area,
        composer,
        is_composer_focused,
        tokens,
        is_mono,
        is_busy,
    );
}

/// Render top status strip with session and runtime telemetry.
fn render_status_strip(f: &mut Frame, area: Rect, model: &TuiViewModel, is_mono: bool) {
    let sid = model.session_id.as_deref().unwrap_or("standalone");
    let mid = model.mission_id.as_deref().unwrap_or("none");
    let is_executing = model.mission_status == "executing";
    let is_running = is_executing
        || model.session_status == "running"
        || model.mission_status == "running"
        || model.session_status == "active";

    let status_str = if is_executing {
        "EXECUTING"
    } else if is_running {
        "RUNNING"
    } else {
        "READY"
    };

    let model_name = if model.active_model.is_empty() || model.active_model == "none" {
        "default"
    } else {
        &model.active_model
    };
    let provider_display = if model.active_provider.is_empty() || model.active_provider == "none" {
        String::new()
    } else {
        format!(" ({})", model.active_provider)
    };
    let branch_display = match (&model.execution_worktree_branch, model.git_branch.as_str()) {
        (Some(exec_branch), ws) if !ws.is_empty() && ws != "N/A" && exec_branch != ws => {
            format!("{exec_branch} (ws: {ws})")
        }
        (Some(exec_branch), _) => exec_branch.clone(),
        (None, "") => "N/A".to_string(),
        (None, ws) => ws.to_string(),
    };

    let status_color = if is_running {
        Color::Green
    } else if model.session_status == "awaiting_approval" {
        Color::Magenta
    } else if model.session_status == "error" {
        Color::Red
    } else {
        Color::Cyan
    };

    let status_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD)
    };

    let spans = if is_running && !model.tasks.is_empty() {
        let total = model.tasks.len();
        let completed = model
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count();
        let active_title = model
            .tasks
            .iter()
            .find(|t| t.status == "running" || t.status == "in_progress")
            .map(|t| t.title.as_str())
            .unwrap_or("executing");
        let task_idx = (completed + 1).min(total);
        let tok_count = model.model_usage.prompt_tokens + model.model_usage.completion_tokens;
        let tok_str = if tok_count >= 1000 {
            format!("{:.1}k tok", tok_count as f64 / 1000.0)
        } else {
            format!("{tok_count} tok")
        };
        let cost_str = match model.model_usage.total_cost_cents {
            Some(cents) => format!("${:.2}", cents as f64 / 100.0),
            None => "cost n/a".to_string(),
        };
        let elapsed_str = format!("{}s", model.system_stats.uptime_secs);

        vec![
            Span::raw(" Session: "),
            Span::styled(sid, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled("● ", status_style),
            Span::styled(format!("[{status_str}] "), status_style),
            Span::raw("· "),
            Span::styled(
                format!("Task {task_idx}/{total} ({active_title}) "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("· "),
            Span::raw(format!("{elapsed_str} · {tok_str} · {cost_str} ")),
            Span::raw("| Branch: "),
            Span::styled(
                &branch_display,
                Style::default().fg(if is_mono { Color::White } else { Color::Green }),
            ),
        ]
    } else {
        vec![
            Span::raw(" Session: "),
            Span::styled(sid, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" | Mission: "),
            Span::raw(mid),
            Span::raw(" | Status: "),
            Span::styled(format!("[{status_str}]"), status_style),
            Span::raw(" | Model: "),
            Span::styled(
                format!("{model_name}{provider_display}"),
                Style::default().fg(if is_mono { Color::White } else { Color::Yellow }),
            ),
            Span::raw(" | Branch: "),
            Span::styled(
                &branch_display,
                Style::default().fg(if is_mono { Color::White } else { Color::Green }),
            ),
        ]
    };

    let text = vec![Line::from(spans)];
    let block = Block::default()
        .title(" Session Cockpit [1] ")
        .borders(Borders::ALL)
        .border_style(if is_mono {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}

/// Render conversation history with badge formatting, live activity, and scrolling.
fn render_timeline_section(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_mono: bool,
) {
    // If live activity is active, reserve 2 lines at the bottom of the timeline area
    let (history_area, activity_area) = if model.live_activity.is_some() {
        let sub = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(2)])
            .split(area);
        (sub[0], Some(sub[1]))
    } else {
        (area, None)
    };

    // Build timeline lines
    let mut all_lines: Vec<Line<'static>> = Vec::new();

    if model.conversation.is_empty() {
        let welcome_style = if is_mono {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let accent_style = if is_mono {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };
        let cmd_style = if is_mono {
            Style::default()
        } else {
            Style::default().fg(Color::Yellow)
        };

        all_lines.push(Line::raw(""));
        all_lines.push(Line::styled(
            "  ◆ M31A Autonomous Software Engineering Agent",
            accent_style,
        ));
        all_lines.push(Line::styled(
            "    The model proposes. The runtime decides.",
            welcome_style,
        ));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::styled(
            "  Start by describing what you want M31A to build, fix, inspect, or explain.",
            if is_mono {
                Style::default()
            } else {
                Style::default().fg(Color::White)
            },
        ));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::from(vec![
            Span::styled("    /help    ", cmd_style),
            Span::styled("Show available commands and usage guide", welcome_style),
        ]));
        all_lines.push(Line::from(vec![
            Span::styled("    /status  ", cmd_style),
            Span::styled(
                "Inspect current workspace, git state, and mission",
                welcome_style,
            ),
        ]));
        all_lines.push(Line::from(vec![
            Span::styled("    /diff    ", cmd_style),
            Span::styled(
                "View unstaged modifications across the repository",
                welcome_style,
            ),
        ]));
        all_lines.push(Line::from(vec![
            Span::styled("    /doctor  ", cmd_style),
            Span::styled(
                "Run 6-category system and environment diagnostics",
                welcome_style,
            ),
        ]));
        all_lines.push(Line::from(vec![
            Span::styled("    @file    ", cmd_style),
            Span::styled(
                "Reference workspace files with real-time completion",
                welcome_style,
            ),
        ]));
        all_lines.push(Line::raw(""));
        all_lines.push(Line::styled(
            "  Universal Shortcuts: [Ctrl+P] Command Palette · [Enter] Send · [Shift+Enter] Multiline · [Esc] Unfocus",
            welcome_style,
        ));
    } else {
        for item in &model.conversation {
            let rendered = item.render_lines(history_area.width.saturating_sub(4), tokens);
            for l in rendered {
                all_lines.push(l);
            }
            // Add blank spacer between turns
            all_lines.push(Line::raw(""));
        }
    }

    // Scrolling logic
    let total_lines = all_lines.len();
    let viewport_height = history_area.height.saturating_sub(2) as usize; // Account for borders

    let scroll_y = if total_lines > viewport_height {
        let max_scroll = total_lines.saturating_sub(viewport_height);
        // model.scroll_offset scrolls up from bottom (0 = pinned to bottom)
        max_scroll.saturating_sub(model.scroll_offset)
    } else {
        0
    };

    let title = format!(
        " Conversation Timeline ({} items) {} ",
        model.conversation.len(),
        if model.scroll_offset > 0 {
            format!("[Scrolled +{}]", model.scroll_offset)
        } else {
            "[Live Bottom]".to_string()
        }
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if is_mono {
            Style::default()
        } else {
            Style::default().fg(Color::Cyan)
        });

    let p = Paragraph::new(all_lines)
        .block(block)
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, history_area);

    // Live Activity Indicator
    if let (Some(act_area), Some(activity_text)) = (activity_area, &model.live_activity) {
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spin_idx = (model.system_stats.uptime_secs % (spinner_chars.len() as u64)) as usize;
        let spin_symbol = spinner_chars[spin_idx];

        let act_line = Line::from(vec![
            Span::styled(
                format!(" {spin_symbol} Agent Activity: "),
                if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(
                activity_text.clone(),
                if is_mono {
                    Style::default()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ]);

        let act_block = Block::default()
            .borders(Borders::ALL)
            .border_style(if is_mono {
                Style::default()
            } else {
                Style::default().fg(Color::Yellow)
            });

        let act_widget = Paragraph::new(act_line).block(act_block);
        f.render_widget(act_widget, act_area);
    }
}

/// Render the bottom interactive prompt composer.
fn render_composer_section(
    f: &mut Frame,
    area: Rect,
    composer: &TuiComposer,
    is_composer_focused: bool,
    tokens: &ThemeTokens,
    is_mono: bool,
    is_busy: bool,
) {
    if is_composer_focused {
        composer.render(f, area, tokens);
    } else {
        // Unfocused or busy presentation
        let title = if is_busy {
            " Composer [Executing · Esc to cancel | Ctrl+C to abort] "
        } else {
            " Composer [Unfocused · Press Enter or 'i' to type | / for commands | @ for files] "
        };
        let border_color = if is_mono {
            Color::White
        } else if is_busy {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let placeholder_text = if is_busy {
            " m31a ❯ Working on task... [Esc to cancel | Ctrl+C to abort]"
        } else if composer.text().is_empty() {
            " m31a> Press Enter or 'i' to focus composer..."
        } else {
            " m31a> (Paused: press Enter or 'i' to resume typing)"
        };

        let p = Paragraph::new(placeholder_text)
            .style(if is_busy {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .block(block);

        f.render_widget(p, area);
    }
}

/// Render live status overview, task summary, and recent logs (COP-01).
fn render_status_overview(f: &mut Frame, area: Rect, model: &TuiViewModel, is_mono: bool) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let mut text = String::new();
    if area.height < 12 {
        text.push_str("MISSION COCKPIT OVERVIEW\n");
        text.push_str(&format!(
            "Tasks Total: {} | Active Agents: {} | Pending Approvals: {}\n",
            model.tasks.len(),
            model.agents.len(),
            model.approvals.len(),
        ));
        text.push_str("Recent Logs:\n");
        for log in model
            .logs
            .iter()
            .rev()
            .take(area.height.saturating_sub(4) as usize)
        {
            text.push_str(&format!(
                " [{}] [{}] {}\n",
                log.level, log.source, log.message
            ));
        }
    } else {
        text.push_str(&format!(
            "MISSION COCKPIT OVERVIEW\n\
             ========================\n\
             Status:           {}\n\
             Objective:        {}\n\
             Tasks Total:      {} (Completed: {})\n\
             Active Agents:    {}\n\
             Events Processed: {}\n\
             Pending Approvals:{}\n\n\
             Recent Logs:\n",
            model.mission_status.to_uppercase(),
            model.objective,
            model.tasks.len(),
            model
                .tasks
                .iter()
                .filter(|t| t.status == "completed")
                .count(),
            model.agents.len(),
            model.system_stats.events_processed,
            model.approvals.len()
        ));
        for log in model
            .logs
            .iter()
            .rev()
            .take(area.height.saturating_sub(10) as usize)
        {
            text.push_str(&format!(
                " [{}] [{}] {}\n",
                log.level, log.source, log.message
            ));
        }
    }

    let block = Block::default()
        .title(" Mission Cockpit Overview ")
        .borders(Borders::ALL)
        .border_style(if is_mono {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}
