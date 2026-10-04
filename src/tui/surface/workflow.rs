//! Workflow Visual Dashboard Surface.
//!
//! Renders live workflow execution state from WorkflowExecutionSnapshot.
//! Conversation-first: opens as detail inspector on TaskGraph screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::tui::theme::{ThemeMode, ThemeTokens};
use crate::workflow::engine::WorkflowExecutionSnapshot;
use crate::workflow::state::{WorkflowMode, WorkflowRunState, WorkflowStepState};

/// Workflow dashboard render mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkflowViewMode {
    #[default]
    Overview,
    StepGraph,
    StepInspector,
    Controls,
}

/// State for workflow dashboard interaction.
#[derive(Debug, Clone, Default)]
pub struct WorkflowDashboardState {
    pub view_mode: WorkflowViewMode,
    pub selected_step_index: usize,
    pub step_list_state: ListState,
    pub show_controls: bool,
}

impl WorkflowDashboardState {
    pub fn new() -> Self {
        let mut state = Self::default();
        state.step_list_state.select(Some(0));
        state
    }
}

/// Render the Workflow Visual Dashboard surface.
pub fn render_workflow_dashboard(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    is_focused: bool,
    ui_state: &mut WorkflowDashboardState,
) {
    if area.width < 20 || area.height < 8 {
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

    // Header with workflow run summary
    let header_height = 6;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(10)])
        .split(area);

    render_workflow_header(
        f,
        chunks[0],
        snapshot,
        tokens,
        border_style,
        ui_state.view_mode,
    );

    // Main content area based on view mode
    match ui_state.view_mode {
        WorkflowViewMode::Overview => render_overview(f, chunks[1], snapshot, tokens, border_style),
        WorkflowViewMode::StepGraph => {
            render_step_graph(f, chunks[1], snapshot, tokens, border_style, ui_state)
        }
        WorkflowViewMode::StepInspector => {
            render_step_inspector(f, chunks[1], snapshot, tokens, border_style, ui_state)
        }
        WorkflowViewMode::Controls => {
            render_controls(f, chunks[1], snapshot, tokens, border_style, ui_state)
        }
    }
}

fn render_workflow_header(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    border_style: Style,
    view_mode: WorkflowViewMode,
) {
    let run = &snapshot.run;
    let status_style = status_style_for_run(run.status, tokens);

    let mode_str = match run.mode {
        WorkflowMode::Standard => "Standard",
        WorkflowMode::Interactive => "Interactive",
        WorkflowMode::Autonomous => "Autonomous",
    };

    let completed_steps = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Completed)
        .count();
    let total_steps = snapshot.step_runs.len();
    let failed_steps = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Failed)
        .count();
    let _pending_steps = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Pending)
        .count();
    let _running_steps = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Running)
        .count();
    let awaiting_approval = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::AwaitingApproval)
        .count();

    let view_tabs = [
        ("Overview", WorkflowViewMode::Overview),
        ("Step Graph", WorkflowViewMode::StepGraph),
        ("Inspector", WorkflowViewMode::StepInspector),
        ("Controls", WorkflowViewMode::Controls),
    ];

    let mut tab_spans = Vec::new();
    for (i, (label, mode)) in view_tabs.iter().enumerate() {
        let is_active = *mode == view_mode;
        let style = if is_active {
            tokens.text_primary
        } else {
            tokens.text_muted
        };
        tab_spans.push(Span::styled(format!(" {} ", label), style));
        if i < view_tabs.len() - 1 {
            tab_spans.push(Span::styled("│", tokens.separator));
        }
    }

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Workflow Run: ", tokens.text_muted),
            Span::styled(
                run.id.to_string(),
                tokens.accent_primary.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Definition: ", tokens.text_muted),
            Span::styled(&run.definition_id, tokens.text_primary),
            Span::raw(" v"),
            Span::styled(run.definition_version.to_string(), tokens.text_secondary),
        ]),
        Line::from(vec![
            Span::styled("Status: ", tokens.text_muted),
            Span::styled(
                run.status.to_string().to_uppercase(),
                status_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Mode: ", tokens.text_muted),
            Span::styled(mode_str, tokens.text_secondary),
            Span::raw("  "),
            Span::styled("Steps: ", tokens.text_muted),
            Span::styled(
                format!("{}/{}", completed_steps, total_steps),
                tokens.accent_primary,
            ),
            if failed_steps > 0 {
                Span::styled(format!("  Failed: {}", failed_steps), tokens.status_failed)
            } else {
                Span::raw("")
            },
            if awaiting_approval > 0 {
                Span::styled(
                    format!("  Awaiting Approval: {}", awaiting_approval),
                    tokens.status_warning,
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("Started: ", tokens.text_muted),
            Span::styled(
                run.started_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                tokens.text_secondary,
            ),
            Span::raw("  "),
            Span::styled("Updated: ", tokens.text_muted),
            Span::styled(
                run.updated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("Workspace: ", tokens.text_muted),
            Span::styled(
                run.workspace_root.display().to_string(),
                tokens.text_secondary,
            ),
        ]),
        Line::raw(""),
        Line::from(tab_spans),
    ];

    if let Some(completed) = run.completed_at {
        lines.push(Line::from(vec![
            Span::styled("Completed: ", tokens.text_muted),
            Span::styled(
                completed.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                tokens.text_secondary,
            ),
        ]));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Workflow ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn status_style_for_run(status: WorkflowRunState, tokens: &ThemeTokens) -> Style {
    match status {
        WorkflowRunState::Pending => tokens.status_waiting,
        WorkflowRunState::Running => tokens.status_running,
        WorkflowRunState::AwaitingInput => tokens.status_warning,
        WorkflowRunState::AwaitingApproval => tokens.status_warning,
        WorkflowRunState::Completed => tokens.status_ok,
        WorkflowRunState::Failed => tokens.status_failed,
        WorkflowRunState::Blocked => tokens.status_warning,
        WorkflowRunState::Cancelled => tokens.text_muted,
        WorkflowRunState::Superseded => tokens.text_muted,
    }
}

fn step_status_style(status: WorkflowStepState, tokens: &ThemeTokens) -> (Style, &'static str) {
    match status {
        WorkflowStepState::Pending => (tokens.status_waiting, "○"),
        WorkflowStepState::Running => (tokens.status_running, "▶"),
        WorkflowStepState::AwaitingApproval => (tokens.status_warning, "⏸"),
        WorkflowStepState::AwaitingInput => (tokens.status_warning, "?"),
        WorkflowStepState::Completed => (tokens.status_ok, "✓"),
        WorkflowStepState::Failed => (tokens.status_failed, "✗"),
        WorkflowStepState::Blocked => (tokens.status_warning, "■"),
        WorkflowStepState::Cancelled => (tokens.text_muted, "⊘"),
        WorkflowStepState::Skipped => (tokens.text_muted, "⏭"),
    }
}

fn render_overview(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    border_style: Style,
) {
    let mut lines = Vec::new();

    lines.push(Line::styled(
        "EXECUTION OVERVIEW",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    // Progress summary
    let total = snapshot.step_runs.len();
    let completed = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Completed)
        .count();
    let running = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Running)
        .count();
    let pending = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Pending)
        .count();
    let blocked = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Blocked)
        .count();
    let awaiting = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::AwaitingApproval)
        .count();
    let failed = snapshot
        .step_runs
        .iter()
        .filter(|s| s.status == WorkflowStepState::Failed)
        .count();

    lines.push(Line::from(vec![
        Span::styled("Total Steps:      ", tokens.text_muted),
        Span::styled(
            total.to_string(),
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ✓ Completed:     ", tokens.status_ok),
        Span::styled(completed.to_string(), tokens.text_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ▶ Running:       ", tokens.status_running),
        Span::styled(running.to_string(), tokens.text_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ○ Pending:       ", tokens.status_waiting),
        Span::styled(pending.to_string(), tokens.text_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ■ Blocked:       ", tokens.status_warning),
        Span::styled(blocked.to_string(), tokens.text_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ⏸ Awaiting Approval:", tokens.status_warning),
        Span::styled(awaiting.to_string(), tokens.text_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  ✗ Failed:        ", tokens.status_failed),
        Span::styled(failed.to_string(), tokens.text_primary),
    ]));

    lines.push(Line::raw(""));

    // Ready / Blocked steps
    if !snapshot.ready_step_keys.is_empty() {
        lines.push(Line::styled(
            "READY TO EXECUTE:",
            tokens.status_ok.add_modifier(Modifier::BOLD),
        ));
        for key in &snapshot.ready_step_keys {
            lines.push(Line::from(vec![
                Span::styled("  ▶ ", tokens.status_ok),
                Span::styled(key, tokens.text_primary),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if !snapshot.blocked_step_keys.is_empty() {
        lines.push(Line::styled(
            "BLOCKED (dependencies unmet):",
            tokens.status_warning.add_modifier(Modifier::BOLD),
        ));
        for key in &snapshot.blocked_step_keys {
            lines.push(Line::from(vec![
                Span::styled("  ■ ", tokens.status_warning),
                Span::styled(key, tokens.text_secondary),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Artifacts summary
    if !snapshot.artifacts.is_empty() {
        lines.push(Line::styled(
            format!("ARTIFACTS PRODUCED ({})", snapshot.artifacts.len()),
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ));
        for art in &snapshot.artifacts {
            let path_str = art.path.display().to_string();
            lines.push(Line::from(vec![
                Span::styled("  • ", tokens.accent_secondary),
                Span::styled(&art.name, tokens.text_primary),
                Span::raw(" ("),
                Span::styled(path_str, tokens.text_muted),
                Span::raw(")"),
            ]));
        }
    } else {
        lines.push(Line::styled(
            "ARTIFACTS PRODUCED (0)",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(
            "  No artifacts recorded yet.",
            tokens.text_muted,
        ));
    }

    if let Some(err) = &snapshot.run.error_summary {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "ERROR:",
            tokens.status_failed.add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(format!("  {}", err), tokens.status_failed));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Overview ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_step_graph(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    border_style: Style,
    ui_state: &mut WorkflowDashboardState,
) {
    let items: Vec<ListItem> = snapshot
        .step_runs
        .iter()
        .map(|step| {
            let (style, icon) = step_status_style(step.status, tokens);
            let attempt_str = if step.attempt_count > 1 {
                format!(" (attempt {})", step.attempt_count)
            } else {
                String::new()
            };
            let _agent_str = step
                .assigned_agent_id
                .map(|a| format!(" @{}", a))
                .unwrap_or_default();
            let mission_str = step
                .mission_id
                .map(|m| format!(" [mission:{}]", m))
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(icon, style),
                Span::styled(format!(" {:<30}", truncate(&step.step_key, 30)), style),
                Span::styled(format!(" {:<18}", format!("{:?}", step.status)), style),
                Span::styled(attempt_str, tokens.text_muted),
                Span::styled(mission_str, tokens.text_muted),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Step Graph (↑/↓ navigate, Enter=inspect) ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut ui_state.step_list_state);
}

fn render_step_inspector(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    border_style: Style,
    ui_state: &WorkflowDashboardState,
) {
    let idx = ui_state
        .selected_step_index
        .min(snapshot.step_runs.len().saturating_sub(1));
    let step = match snapshot.step_runs.get(idx) {
        Some(s) => s,
        None => {
            let p = Paragraph::new("No step selected").block(
                Block::default()
                    .borders(Borders::NONE)
                    .title(" Step Inspector ")
                    .border_style(border_style),
            );
            f.render_widget(p, area);
            return;
        }
    };

    let (status_style, status_icon) = step_status_style(step.status, tokens);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Step: ", tokens.text_muted),
        Span::styled(
            &step.step_key,
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(status_icon, status_style),
        Span::styled(
            format!(" {:?}", step.status),
            status_style.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Basic info
    lines.push(Line::from(vec![
        Span::styled("Step Run ID:   ", tokens.text_muted),
        Span::styled(step.id.to_string(), tokens.text_secondary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Workflow Run:  ", tokens.text_muted),
        Span::styled(step.workflow_run_id.to_string(), tokens.text_secondary),
    ]));
    if let Some(mission_id) = step.mission_id {
        lines.push(Line::from(vec![
            Span::styled("Mission ID:    ", tokens.text_muted),
            Span::styled(mission_id.to_string(), tokens.text_secondary),
        ]));
    }
    if let Some(agent_id) = step.assigned_agent_id {
        lines.push(Line::from(vec![
            Span::styled("Assigned Agent:", tokens.text_muted),
            Span::styled(agent_id.to_string(), tokens.text_secondary),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("Attempt Count: ", tokens.text_muted),
        Span::styled(step.attempt_count.to_string(), tokens.text_primary),
    ]));

    lines.push(Line::raw(""));

    // Timestamps
    lines.push(Line::from(vec![
        Span::styled("Started:       ", tokens.text_muted),
        Span::styled(
            step.started_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            tokens.text_secondary,
        ),
    ]));
    if let Some(completed) = step.completed_at {
        lines.push(Line::from(vec![
            Span::styled("Completed:     ", tokens.text_muted),
            Span::styled(
                completed.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                tokens.text_secondary,
            ),
        ]));
    }
    if let Some(halt) = &step.halt_reason {
        lines.push(Line::from(vec![
            Span::styled("Halt Reason:   ", tokens.status_failed),
            Span::styled(halt, tokens.text_secondary),
        ]));
    }

    lines.push(Line::raw(""));

    // Artifacts for this step
    let step_artifacts: Vec<_> = snapshot
        .artifacts
        .iter()
        .filter(|a| a.step_run_id == step.id)
        .collect();
    if !step_artifacts.is_empty() {
        lines.push(Line::styled(
            "ARTIFACTS:",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ));
        for art in step_artifacts {
            let path_str = art.path.display().to_string();
            lines.push(Line::from(vec![
                Span::styled("  • ", tokens.accent_secondary),
                Span::styled(&art.name, tokens.text_primary),
                Span::raw(" ("),
                Span::styled(path_str, tokens.text_muted),
                Span::raw(")"),
            ]));
        }
    } else {
        lines.push(Line::styled("ARTIFACTS: (none)", tokens.text_muted));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Navigation: [Tab] Overview | [G] Graph | [C] Controls | [Esc] Back",
        tokens.text_muted,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(format!(" Step Inspector [{}] ", step.step_key))
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_controls(
    f: &mut Frame,
    area: Rect,
    snapshot: &WorkflowExecutionSnapshot,
    tokens: &ThemeTokens,
    border_style: Style,
    _ui_state: &WorkflowDashboardState,
) {
    let run = &snapshot.run;
    let can_resume = run.status == WorkflowRunState::Blocked;
    let can_pause = run.status == WorkflowRunState::Running;
    let can_cancel = !run.status.is_terminal();
    let has_awaiting_approval = snapshot
        .step_runs
        .iter()
        .any(|s| s.status == WorkflowStepState::AwaitingApproval);

    let mut lines = vec![
        Line::styled(
            "WORKFLOW CONTROLS",
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled("Available actions for current state:", tokens.text_muted),
        Line::raw(""),
    ];

    // Resume
    let resume_style = if can_resume {
        tokens.status_ok
    } else {
        tokens.text_muted
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  [R] Resume          ",
            if can_resume {
                resume_style.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_muted
            },
        ),
        Span::styled(
            if can_resume {
                "✓ Resume paused/blocked workflow"
            } else {
                "✗ Only available when Blocked/Paused"
            },
            resume_style,
        ),
    ]));

    // Pause
    let pause_style = if can_pause {
        tokens.status_warning
    } else {
        tokens.text_muted
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  [P] Pause           ",
            if can_pause {
                pause_style.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_muted
            },
        ),
        Span::styled(
            if can_pause {
                "✓ Pause running workflow"
            } else {
                "✗ Only available when Running"
            },
            pause_style,
        ),
    ]));

    // Cancel
    let cancel_style = if can_cancel {
        tokens.status_failed
    } else {
        tokens.text_muted
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  [X] Cancel          ",
            if can_cancel {
                cancel_style.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_muted
            },
        ),
        Span::styled(
            if can_cancel {
                "✓ Cancel workflow (stops all steps)"
            } else {
                "✗ Workflow already terminal"
            },
            cancel_style,
        ),
    ]));

    // Approve
    let approve_style = if has_awaiting_approval {
        tokens.status_ok
    } else {
        tokens.text_muted
    };
    lines.push(Line::from(vec![
        Span::styled(
            "  [A] Approve Step    ",
            if has_awaiting_approval {
                approve_style.add_modifier(Modifier::BOLD)
            } else {
                tokens.text_muted
            },
        ),
        Span::styled(
            if has_awaiting_approval {
                "✓ Approve awaiting-approval step"
            } else {
                "✗ No step awaiting approval"
            },
            approve_style,
        ),
    ]));

    // Inspect (always available)
    lines.push(Line::from(vec![
        Span::styled(
            "  [I] Inspect         ",
            tokens.status_ok.add_modifier(Modifier::BOLD),
        ),
        Span::styled("✓ View detailed workflow snapshot", tokens.status_ok),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled("Action routing:", tokens.text_muted));
    lines.push(Line::styled(
        "  TUI → ApplicationAction → AppRuntime → WorkflowEngine → EventBus → TUI projection",
        tokens.text_muted,
    ));
    lines.push(Line::raw(""));
    lines.push(Line::styled("Current workflow state:", tokens.text_muted));
    lines.push(Line::from(vec![
        Span::styled("  Status: ", tokens.text_muted),
        Span::styled(
            run.status.to_string(),
            status_style_for_run(run.status, tokens).add_modifier(Modifier::BOLD),
        ),
    ]));
    if let Some(err) = &run.error_summary {
        lines.push(Line::from(vec![
            Span::styled("  Error:  ", tokens.status_failed),
            Span::styled(err, tokens.status_failed),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Navigation: [Tab] Overview | [G] Graph | [I] Inspector | [Esc] Back",
        tokens.text_muted,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Controls ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}

/// Handle keyboard input for workflow dashboard.
pub fn handle_workflow_dashboard_key(
    key: crossterm::event::KeyEvent,
    ui_state: &mut WorkflowDashboardState,
    snapshot: &WorkflowExecutionSnapshot,
) -> Option<WorkflowAction> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Tab => {
            ui_state.view_mode = match ui_state.view_mode {
                WorkflowViewMode::Overview => WorkflowViewMode::StepGraph,
                WorkflowViewMode::StepGraph => WorkflowViewMode::StepInspector,
                WorkflowViewMode::StepInspector => WorkflowViewMode::Controls,
                WorkflowViewMode::Controls => WorkflowViewMode::Overview,
            };
            None
        }
        KeyCode::Char('g') | KeyCode::Char('G') => {
            ui_state.view_mode = WorkflowViewMode::StepGraph;
            None
        }
        KeyCode::Char('i') | KeyCode::Char('I') => {
            ui_state.view_mode = WorkflowViewMode::StepInspector;
            None
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            ui_state.view_mode = WorkflowViewMode::Controls;
            None
        }
        KeyCode::Char('o') | KeyCode::Char('O') => {
            ui_state.view_mode = WorkflowViewMode::Overview;
            None
        }
        KeyCode::Up => {
            if ui_state.view_mode == WorkflowViewMode::StepGraph && ui_state.selected_step_index > 0
            {
                ui_state.selected_step_index -= 1;
                ui_state
                    .step_list_state
                    .select(Some(ui_state.selected_step_index));
            }
            None
        }
        KeyCode::Down => {
            if ui_state.view_mode == WorkflowViewMode::StepGraph {
                let count = snapshot.step_runs.len();
                if ui_state.selected_step_index + 1 < count {
                    ui_state.selected_step_index += 1;
                    ui_state
                        .step_list_state
                        .select(Some(ui_state.selected_step_index));
                }
            }
            None
        }
        KeyCode::Enter => {
            if ui_state.view_mode == WorkflowViewMode::StepGraph {
                ui_state.view_mode = WorkflowViewMode::StepInspector;
            }
            None
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if snapshot.run.status == WorkflowRunState::Blocked {
                Some(WorkflowAction::Resume)
            } else {
                None
            }
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            if snapshot.run.status == WorkflowRunState::Running {
                Some(WorkflowAction::Pause)
            } else {
                None
            }
        }
        KeyCode::Char('x') | KeyCode::Char('X') => {
            if !snapshot.run.status.is_terminal() {
                Some(WorkflowAction::Cancel)
            } else {
                None
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => snapshot
            .step_runs
            .iter()
            .find(|s| s.status == WorkflowStepState::AwaitingApproval)
            .map(|step| WorkflowAction::Approve(step.step_key.clone())),
        KeyCode::Esc => Some(WorkflowAction::Close),
        _ => None,
    }
}

/// Actions that can be triggered from the workflow dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowAction {
    Resume,
    Pause,
    Cancel,
    Approve(String), // step_key
    Close,
}
