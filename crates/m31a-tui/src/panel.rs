//! Contextual Workspace Panel for M31A TUI.
//!
//! Provides an on-demand, resizable, tabbed side panel containing:
//! 1. Files: Workspace directory tree and file listing.
//! 2. Git: Uncommitted working tree changes, modified/added/deleted files, and branch info.
//! 3. Diff: Interactive diff view of current workspace changes.
//! 4. Terminal: Background processes, status, and command execution visibility.
//! 5. Context: Pinned dynamic context items and memory digests.
//! 6. Activity: Execution timeline, recent tool calls, and causal critical path timings.
//! 7. Subagents: Child agent tree, status, and communication board.
//! 8. Diagnostics: Token consumption, latency, concurrency locks, and system health.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use crate::theme::{ColorRole, Theme};

/// The eight dedicated sections of the contextual workspace panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelTab {
    #[default]
    Files = 0,
    Git = 1,
    Diff = 2,
    Terminal = 3,
    Context = 4,
    Activity = 5,
    Subagents = 6,
    Diagnostics = 7,
}

impl PanelTab {
    pub const ALL: [PanelTab; 8] = [
        PanelTab::Files,
        PanelTab::Git,
        PanelTab::Diff,
        PanelTab::Terminal,
        PanelTab::Context,
        PanelTab::Activity,
        PanelTab::Subagents,
        PanelTab::Diagnostics,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            PanelTab::Files => "Files",
            PanelTab::Git => "Git",
            PanelTab::Diff => "Diff",
            PanelTab::Terminal => "Terminal",
            PanelTab::Context => "Context",
            PanelTab::Activity => "Activity",
            PanelTab::Subagents => "Subagents",
            PanelTab::Diagnostics => "Diagnostics",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PanelTab::Files => "▸",
            PanelTab::Git => "⎇",
            PanelTab::Diff => "±",
            PanelTab::Terminal => "❯_",
            PanelTab::Context => "§",
            PanelTab::Activity => "◷",
            PanelTab::Subagents => "◇",
            PanelTab::Diagnostics => "~",
        }
    }
}

/// Renders the contextual workspace panel with tab bar, content region, and shortcut footer.
pub fn render_contextual_panel(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    if area.width < 12 || area.height < 5 {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.default)
        .border_style(t.border_focus())
        .title(Line::from(vec![
            Span::styled(
                " Workspace Panel ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{}] ", app.context_panel_tab.title()),
                t.style_accent(true),
            ),
            Span::styled("(Ctrl+B to close) ", t.style_muted()),
        ]))
        .style(t.style_surface1());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    // Split vertically: Tab selector (height 2), Content (Min 3), Footer hint (height 1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Tab bar
            Constraint::Min(3),    // Active tab content
            Constraint::Length(1), // Footer navigation hint
        ])
        .split(inner);

    render_tab_bar(f, app, chunks[0], t);
    render_tab_content(f, app, chunks[1], t);
    render_panel_footer(f, app, chunks[2], t);
}

/// Renders the top tab bar for the contextual panel.
fn render_tab_bar(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));

    for tab in &PanelTab::ALL {
        let is_active = app.context_panel_tab == *tab;
        let title = tab.title();

        let style = if is_active {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(t.color(ColorRole::TextMuted))
                .bg(t.color(ColorRole::Surface0))
        };

        spans.push(Span::styled(format!(" {} ", title), style));
        spans.push(Span::raw(" "));
    }

    let p = Paragraph::new(Line::from(spans))
        .style(t.style_surface0())
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(t.border_subtle()),
        );
    f.render_widget(p, area);
}

/// Renders the active tab content within the contextual panel.
fn render_tab_content(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    match app.context_panel_tab {
        PanelTab::Files => render_files_tab(f, app, area, t),
        PanelTab::Git => render_git_tab(f, app, area, t),
        PanelTab::Diff => render_diff_tab(f, app, area, t),
        PanelTab::Terminal => render_terminal_tab(f, app, area, t),
        PanelTab::Context => render_context_tab(f, app, area, t),
        PanelTab::Activity => render_activity_tab(f, app, area, t),
        PanelTab::Subagents => render_subagents_tab(f, app, area, t),
        PanelTab::Diagnostics => render_diagnostics_tab(f, app, area, t),
    }
}

/// Renders the panel footer showing shortcut hints.
fn render_panel_footer(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let hint = format!(
        " [ / ] Tab  Alt+[/] Width:{}%  Ctrl+B Close ",
        app.context_panel_width_pct
    );
    let p = Paragraph::new(Line::from(vec![Span::styled(hint, t.style_muted())]))
        .style(t.style_surface0());
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 1. FILES TAB
// -------------------------------------------------------------------------------------------------
fn render_files_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let candidates = m31a_fuzzy::WorkspaceScanner::collect_workspace_files_cached(&app.cwd, 40);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("Workspace: {}", app.cwd.display()),
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    if candidates.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No files discovered in workspace.",
            t.style_muted(),
        )));
    } else {
        for cand in candidates
            .iter()
            .take(area.height.saturating_sub(3) as usize)
        {
            let (icon, color) = match cand.candidate_type {
                m31a_fuzzy::CandidateType::Directory => ("▸ ", Color::LightBlue),
                m31a_fuzzy::CandidateType::Symbol => ("⚙ ", Color::LightYellow),
                _ => {
                    if cand.insert_text.ends_with(".rs") {
                        ("⟨/⟩ ", Color::Cyan)
                    } else if cand.insert_text.ends_with(".toml")
                        || cand.insert_text.ends_with(".json")
                    {
                        ("{} ", Color::Yellow)
                    } else if cand.insert_text.ends_with(".md") {
                        ("≡ ", Color::Green)
                    } else {
                        ("· ", Color::White)
                    }
                }
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                Span::styled(cand.insert_text.clone(), Style::default().fg(color)),
            ]));
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 2. GIT CHANGES TAB
// -------------------------------------------------------------------------------------------------
fn render_git_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();

    // Branch info
    let branch_str = if let Some(ref git) = app.bottom_pane.git_status {
        format!(
            "Branch: {} {}",
            git.branch,
            if git.is_dirty { "(dirty)" } else { "(clean)" }
        )
    } else {
        "Branch: main".to_string()
    };

    lines.push(Line::from(vec![
        Span::styled(" ", t.style_accent(true)),
        Span::styled(branch_str, t.style_primary().add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::raw(""));

    let git_status_output = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(&app.cwd)
        .output();

    match git_status_output {
        Ok(out) if out.status.success() => {
            let stdout_str = String::from_utf8_lossy(&out.stdout).to_string();
            let change_lines: Vec<&str> = stdout_str.lines().collect();
            if change_lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  ✔ Working directory clean",
                    t.style_success(false),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  Changed files ({}):", change_lines.len()),
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::raw(""));

                for change in change_lines
                    .iter()
                    .take(area.height.saturating_sub(5) as usize)
                {
                    let (status_code, style) =
                        if change.starts_with(" M") || change.starts_with('M') {
                            (
                                "M",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if change.starts_with("??") || change.starts_with('A') {
                            (
                                "A",
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if change.starts_with('D') {
                            (
                                "D",
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            )
                        } else {
                            ("•", t.style_muted())
                        };

                    let path_text = change
                        .trim_start_matches(|c: char| {
                            c.is_alphabetic() || c == '?' || c.is_whitespace()
                        })
                        .trim()
                        .to_string();
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(format!("[{}] ", status_code), style),
                        Span::styled(path_text, t.style_primary()),
                    ]));
                }
            }
        }
        _ => {
            lines.push(Line::from(Span::styled(
                "  Git status unavailable",
                t.style_muted(),
            )));
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 3. DIFF VIEWER TAB
// -------------------------------------------------------------------------------------------------
fn render_diff_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "± Uncommitted Diff",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    let git_diff_output = std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(&app.cwd)
        .output();

    match git_diff_output {
        Ok(out) if out.status.success() => {
            let stdout_str = String::from_utf8_lossy(&out.stdout).to_string();
            let diff_lines: Vec<&str> = stdout_str.lines().collect();
            if diff_lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No unstaged diff detected.",
                    t.style_muted(),
                )));
            } else {
                for l in diff_lines
                    .iter()
                    .take(area.height.saturating_sub(4) as usize)
                {
                    let style = if l.contains('+') && l.contains('-') {
                        Style::default().fg(Color::Yellow)
                    } else if l.contains('+') {
                        Style::default().fg(Color::Green)
                    } else if l.contains('-') {
                        Style::default().fg(Color::Red)
                    } else {
                        t.style_muted()
                    };
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled((*l).to_string(), style),
                    ]));
                }
            }
        }
        _ => {
            lines.push(Line::from(Span::styled(
                "  Diff unavailable (not a git repo or git not found)",
                t.style_muted(),
            )));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Press Ctrl+D or type /diff for full interactive diff.",
        t.style_muted().add_modifier(Modifier::ITALIC),
    )));

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 4. TERMINAL & PROCESSES TAB
// -------------------------------------------------------------------------------------------------
fn render_terminal_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "⚡ Background Tasks & Processes",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    let proc_count = app.proc_supervisor.process_count();
    lines.push(Line::from(vec![
        Span::styled("  Supervised Processes: ", t.style_secondary()),
        Span::styled(
            format!("{} active", proc_count),
            t.style_primary().add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Interactive Commands:",
        t.style_secondary(),
    )));
    lines.push(Line::from(Span::styled(
        "  • /procs   - Open interactive process manager modal",
        t.style_muted(),
    )));
    lines.push(Line::from(Span::styled(
        "  • /logs    - View streaming logs from active background task",
        t.style_muted(),
    )));
    lines.push(Line::from(Span::styled(
        "  • !command - Execute instant subshell command",
        t.style_muted(),
    )));

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 5. CONTEXT & MEMORY TAB
// -------------------------------------------------------------------------------------------------
fn render_context_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "§ Dynamic Context Items",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    if app.pinned_context.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No items currently pinned in context.",
            t.style_muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  Pin files via `/context pin <path>` or `@file` mentions.",
            t.style_muted(),
        )));
    } else {
        for (idx, item) in app.pinned_context.iter().enumerate().take(6) {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}. § ", idx + 1), t.style_accent(false)),
                Span::styled(&item.path, t.style_primary().add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({} tok)", item.tokens), t.style_muted()),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("World State Digests: ", t.style_secondary()),
        Span::styled(
            format!("{} digests indexed", app.world_state_digests.len()),
            t.style_muted(),
        ),
    ]));

    // Token meter summary
    if let Some(ref meter) = app.bottom_pane.token_meter {
        lines.push(Line::from(vec![
            Span::styled("Context Window: ", t.style_secondary()),
            Span::styled(
                format!(
                    "{} / {} tokens ({:.0}%)",
                    meter.used_tokens,
                    meter.limit_tokens,
                    meter.percentage()
                ),
                t.style_info(false),
            ),
        ]));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 6. ACTIVITY & CAUSAL TIMINGS TAB
// -------------------------------------------------------------------------------------------------
fn render_activity_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "⏱ Execution Activity",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    let completed_work = app.direct_tool_timings.completed_work();
    if completed_work.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No tool executions recorded in this turn.",
            t.style_muted(),
        )));
    } else {
        for work in completed_work
            .iter()
            .rev()
            .take(area.height.saturating_sub(6) as usize)
        {
            let dur_ms = work.duration.as_millis();
            let status_icon = if !work.is_error { "✔" } else { "✖" };
            let status_style = if !work.is_error {
                t.style_success(false)
            } else {
                t.style_error(false)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", status_icon), status_style),
                Span::styled(
                    format!("{:<16} ", work.name),
                    t.style_primary().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{}ms", dur_ms), t.style_muted()),
            ]));
        }
    }

    if let Some(ref path) = app.last_turn_critical_path {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Critical Path: ", t.style_secondary()),
            Span::styled(
                format!(
                    "Inference: {}ms │ Tools: {}ms │ Review: {}ms",
                    path.inference.as_millis(),
                    path.tools.as_millis(),
                    path.auto_review.as_millis()
                ),
                t.style_muted(),
            ),
        ]));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 7. SUBAGENTS TAB
// -------------------------------------------------------------------------------------------------
fn render_subagents_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "◇ Subagents & Swarm Hierarchy",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    let subagents = app
        .storage
        .list_subagents(&app.session_id)
        .unwrap_or_default();
    if subagents.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No child subagents active in this session.",
            t.style_muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  Delegate tasks via the `spawn_subagent` tool.",
            t.style_muted(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  Active Children ({}):", subagents.len()),
            t.style_secondary(),
        )));
        for (child_id, role) in subagents.iter().take(5) {
            lines.push(Line::from(vec![
                Span::styled("  ● ", t.style_accent(false)),
                Span::styled(role.clone(), t.style_primary().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" ({})", &child_id[..child_id.len().min(8)]),
                    t.style_muted(),
                ),
            ]));
        }
    }

    // Swarm board posts
    let recent_posts = app
        .storage
        .list_posts("default", "dev", 4)
        .unwrap_or_default();
    if !recent_posts.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "  Recent Swarm Messages:",
            t.style_secondary(),
        )));
        for post in recent_posts {
            lines.push(Line::from(vec![
                Span::styled(format!("  [{}] ", post.author), t.style_accent(false)),
                Span::styled(
                    post.body.lines().next().unwrap_or("").to_string(),
                    t.style_muted(),
                ),
            ]));
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// -------------------------------------------------------------------------------------------------
// 8. DIAGNOSTICS TAB
// -------------------------------------------------------------------------------------------------
fn render_diagnostics_tab(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "~ Session Diagnostics",
        t.style_secondary().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::raw(""));

    let decision = app.trust_store.lock().unwrap().evaluate(&app.cwd);
    let trust_str = match decision {
        m31a_trust::TrustDecision::Trusted => "Trusted",
        m31a_trust::TrustDecision::SessionOnly => "Session Only",
        m31a_trust::TrustDecision::RequiresConfirmation { .. } => "Requires Confirmation",
        m31a_trust::TrustDecision::Denied { .. } => "Denied",
    };

    lines.push(Line::from(vec![
        Span::styled("  Active Model:   ", t.style_secondary()),
        Span::styled(&app.active_model, t.style_accent(true)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Trust Decision: ", t.style_secondary()),
        Span::styled(trust_str, t.style_primary()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Sandbox Policy: ", t.style_secondary()),
        Span::styled(format!("{:?}", app.config.sandbox), t.style_info(false)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Total Tokens:   ", t.style_secondary()),
        Span::styled(format!("{} tok", app.tokens_used), t.style_primary()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Prompt Queue:   ", t.style_secondary()),
        Span::styled(
            format!("{} prompts queued", app.queued_prompts.len()),
            t.style_muted(),
        ),
    ]));

    let active_locks = m31a_core::ExecutionLockManager::global().active_locks();
    lines.push(Line::from(vec![
        Span::styled("  Active Locks:   ", t.style_secondary()),
        Span::styled(
            format!("{} path locks", active_locks.len()),
            t.style_muted(),
        ),
    ]));

    if let Some(ref msg) = app.provider_status_message {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("  Provider Status: ", t.style_secondary()),
            Span::styled(msg, t.style_warning(false)),
        ]));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
