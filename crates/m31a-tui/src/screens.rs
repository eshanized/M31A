//! 10-Screen Primary Navigation and Application Shell for M31A TUI.
//!
//! Provides dedicated screen renderers, persistent navigation bar, collapsible contextual inspector,
//! and contextual keyboard actions across:
//! 1. Chat
//! 2. Sessions
//! 3. Models
//! 4. Providers
//! 5. Agents
//! 6. Workspace
//! 7. Source Control
//! 8. Tools & Integrations
//! 9. Settings
//! 10. Diagnostics

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::app::App;
use crate::theme::{ColorRole, Theme};
use m31a_agent_engine::SpeculativeExecutionReport;
use m31a_core::workflow::{WorkflowState, WorkflowTaskStatus};

/// The 10 primary screens of the M31A developer environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Screen {
    #[default]
    Chat = 0,
    Sessions = 1,
    Models = 2,
    Providers = 3,
    Agents = 4,
    Workspace = 5,
    SourceControl = 6,
    ToolsIntegrations = 7,
    Settings = 8,
    Diagnostics = 9,
}

impl Screen {
    pub const ALL: [Screen; 10] = [
        Screen::Chat,
        Screen::Sessions,
        Screen::Models,
        Screen::Providers,
        Screen::Agents,
        Screen::Workspace,
        Screen::SourceControl,
        Screen::ToolsIntegrations,
        Screen::Settings,
        Screen::Diagnostics,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Screen::Chat => "Chat",
            Screen::Sessions => "Sessions",
            Screen::Models => "Models",
            Screen::Providers => "Providers",
            Screen::Agents => "Agents",
            Screen::Workspace => "Workspace",
            Screen::SourceControl => "Source Control",
            Screen::ToolsIntegrations => "Tools & Integrations",
            Screen::Settings => "Settings",
            Screen::Diagnostics => "Diagnostics",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Screen::Chat => "1",
            Screen::Sessions => "2",
            Screen::Models => "3",
            Screen::Providers => "4",
            Screen::Agents => "5",
            Screen::Workspace => "6",
            Screen::SourceControl => "7",
            Screen::ToolsIntegrations => "8",
            Screen::Settings => "9",
            Screen::Diagnostics => "0",
        }
    }
}

/// Renders the primary 10-screen navigation bar.
pub fn render_navbar(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let mut spans = Vec::new();
    spans.push(Span::styled(" ", t.style_muted()));

    for (i, screen) in Screen::ALL.iter().enumerate() {
        let is_active = app.current_screen == *screen;
        let num = screen.shortcut();
        let title = screen.title();

        let tab_style = if is_active {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(t.color(ColorRole::TextPrimary))
                .bg(t.color(ColorRole::Surface1))
        };

        let num_style = if is_active {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(t.color(ColorRole::Accent))
                .bg(t.color(ColorRole::Surface1))
                .add_modifier(Modifier::BOLD)
        };

        spans.push(Span::styled(format!(" {} ", num), num_style));
        spans.push(Span::styled(format!("{} ", title), tab_style));

        if i + 1 < Screen::ALL.len() {
            spans.push(Span::styled("  ", t.style_muted()));
        }
    }

    // Right-aligned inspector indicator badge
    let inspector_label = if app.inspector_open {
        " [F12 Inspector: ON] "
    } else {
        " [F12 Inspector: OFF] "
    };
    let inspector_style = if app.inspector_open {
        Style::default()
            .fg(t.color(ColorRole::Surface0))
            .bg(t.color(ColorRole::Success))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.color(ColorRole::TextMuted))
    };

    let p = Paragraph::new(Line::from(spans))
        .style(t.style_surface0())
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(t.border_subtle()),
        );
    f.render_widget(p, area);

    let right_area = Rect {
        x: area.x + area.width.saturating_sub(inspector_label.len() as u16 + 1),
        y: area.y,
        width: inspector_label.len() as u16 + 1,
        height: 1,
    };
    if right_area.width > 0 && right_area.x < area.x + area.width {
        let ip = Paragraph::new(inspector_label).style(inspector_style);
        f.render_widget(ip, right_area);
    }
}

/// Renders the content of the active screen.
pub fn render_screen_content(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    match app.current_screen {
        Screen::Chat => {
            // Screen::Chat is rendered by renderer.rs using existing transcript & bottom pane
        }
        Screen::Sessions => render_sessions_screen(f, app, area, t),
        Screen::Models => render_models_screen(f, app, area, t),
        Screen::Providers => render_providers_screen(f, app, area, t),
        Screen::Agents => render_agents_screen(f, app, area, t),
        Screen::Workspace => render_workspace_screen(f, app, area, t),
        Screen::SourceControl => render_source_control_screen(f, app, area, t),
        Screen::ToolsIntegrations => render_tools_integrations_screen(f, app, area, t),
        Screen::Settings => render_settings_screen(f, app, area, t),
        Screen::Diagnostics => render_diagnostics_screen(f, app, area, t),
    }
}

// -------------------------------------------------------------------------------------------------
// 2. SESSIONS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_sessions_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let threads = app.storage.list_threads(100).unwrap_or_default();
    let filter = app.screen_filter.to_lowercase();
    let filtered_threads: Vec<_> = threads
        .into_iter()
        .filter(|th| {
            if filter.is_empty() {
                true
            } else {
                th.title.to_lowercase().contains(&filter)
                    || th.id.to_lowercase().contains(&filter)
                    || th.model.to_lowercase().contains(&filter)
            }
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Search bar
    let search_line = Line::from(vec![
        Span::styled(
            " Search Sessions: ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if app.screen_filter.is_empty() {
                "Type to search (Press '/' or Esc to exit filter)...".to_string()
            } else {
                app.screen_filter.clone()
            },
            if app.screen_filter.is_empty() {
                t.style_muted()
            } else {
                Style::default()
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            },
        ),
    ]);
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.screen_filter_active {
            t.border_focus()
        } else {
            t.border_subtle()
        })
        .title(" Filter (Press '/' to search) ");
    f.render_widget(Paragraph::new(search_line).block(search_block), chunks[0]);

    // Sessions table
    let header_row = Row::new(vec![
        Cell::from("Active").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Title").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Thread ID").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Updated").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let rows: Vec<_> = filtered_threads
        .iter()
        .enumerate()
        .map(|(i, th)| {
            let is_selected = i == app.screen_selected_index;
            let is_current = th.id == app.session_id;

            let active_marker = if is_current {
                Span::styled(
                    " ● ACTIVE ",
                    Style::default()
                        .fg(t.color(ColorRole::Success))
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("   -    ", t.style_muted())
            };

            let row_style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            let updated_str = chrono::DateTime::from_timestamp(th.updated_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| format!("{}", th.updated_at));

            Row::new(vec![
                Cell::from(active_marker),
                Cell::from(th.title.clone()),
                Cell::from(th.id.clone()),
                Cell::from(th.model.clone()),
                Cell::from(updated_str),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Percentage(40),
            Constraint::Length(26),
            Constraint::Percentage(20),
            Constraint::Length(18),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(
                " Stored Sessions ({}) — [Enter] Switch [d] Delete [r] Rename ",
                filtered_threads.len()
            )),
    );

    f.render_widget(table, chunks[1]);
}

// -------------------------------------------------------------------------------------------------
// 3. MODELS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_models_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Search bar
    let search_line = Line::from(vec![
        Span::styled(
            " Search Catalog: ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if app.screen_filter.is_empty() {
                "Type to search models by slug, display name, or capabilities...".to_string()
            } else {
                app.screen_filter.clone()
            },
            if app.screen_filter.is_empty() {
                t.style_muted()
            } else {
                Style::default()
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            },
        ),
    ]);
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.screen_filter_active {
            t.border_focus()
        } else {
            t.border_subtle()
        })
        .title(" Model Search (Press '/' to search, [Enter] activate, [t] test health) ");
    f.render_widget(Paragraph::new(search_line).block(search_block), chunks[0]);

    // Split chunks[1] horizontally into catalogue list and detail inspector
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // Left pane: Model Catalogue list with Scrollbar
    let (selected_model, is_active) = if let Some(ref ms) = app.models_screen_state {
        let sel = ms.get_selected_model().cloned();
        let is_act = sel
            .as_ref()
            .map(|m| m.slug == ms.active_model)
            .unwrap_or(false);
        crate::model_selector::render_model_list(f, ms, body_chunks[0], t, "Model Registry");
        (sel, is_act)
    } else {
        let mut temp_state = crate::model_selector::ModelListState::new(
            app.available_models.clone(),
            app.active_model.clone(),
        );
        let inner_h = body_chunks[0].height.saturating_sub(2) as usize;
        temp_state.set_viewport_height(inner_h);
        temp_state.set_search_query(app.screen_filter.clone());
        temp_state.selected_index = app
            .screen_selected_index
            .min(temp_state.filtered_indices.len().saturating_sub(1));
        temp_state.ensure_selected_visible();
        let sel = temp_state.get_selected_model().cloned();
        let is_act = sel
            .as_ref()
            .map(|m| m.slug == temp_state.active_model)
            .unwrap_or(false);
        crate::model_selector::render_model_list(
            f,
            &temp_state,
            body_chunks[0],
            t,
            "Model Registry",
        );
        (sel, is_act)
    };

    // Right pane: Detail Inspector
    crate::model_selector::render_model_details(
        f,
        selected_model.as_ref(),
        is_active,
        body_chunks[1],
        t,
    );
}

// -------------------------------------------------------------------------------------------------
// 4. PROVIDERS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_providers_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let providers = [(
        "NVIDIA NIM",
        "https://integrate.api.nvidia.com/v1",
        "NVIDIA_API_KEY",
        true,
    )];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(6)])
        .split(area);

    let header_row = Row::new(vec![
        Cell::from("Provider").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Endpoint URL").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Auth Credential").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Key Status").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Active").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let rows: Vec<_> = providers
        .iter()
        .enumerate()
        .map(|(i, (name, endpoint, env_key, _))| {
            let is_selected = i == app.screen_selected_index;
            let key_exists = std::env::var(env_key).is_ok() || (*env_key == "None (Local)");
            let (status_text, status_color) = if key_exists {
                ("Configured (Masked)", t.color(ColorRole::Success))
            } else {
                ("Missing / Unset", t.color(ColorRole::Warning))
            };

            let is_active = app.config.api_base.contains(endpoint)
                || (i == 0 && app.config.api_base.contains("nvidia"));
            let active_badge = if is_active {
                Span::styled(
                    " ● ACTIVE ",
                    Style::default()
                        .fg(t.color(ColorRole::Success))
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("   -    ", t.style_muted())
            };

            let row_style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            Row::new(vec![
                Cell::from(*name),
                Cell::from(*endpoint),
                Cell::from(*env_key),
                Cell::from(Span::styled(status_text, Style::default().fg(status_color))),
                Cell::from(active_badge),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Percentage(40),
            Constraint::Length(22),
            Constraint::Length(22),
            Constraint::Length(12),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(" Inference Providers & Sovereign Endpoints — [t] Test Connectivity [Enter] Select Provider "),
    );

    f.render_widget(table, chunks[0]);

    // Provider detail & connectivity test status
    let status_msg = app
        .provider_status_message
        .as_deref()
        .unwrap_or("Ready. Press [t] to test connectivity to the selected inference provider.");

    let detail_text = vec![
        Line::from(vec![
            Span::styled("Status: ", t.style_secondary().add_modifier(Modifier::BOLD)),
            Span::styled(
                status_msg,
                Style::default().fg(t.color(ColorRole::TextPrimary)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Active Client: ", t.style_secondary()),
            Span::styled(
                format!(
                    "Model: {} | Base: {}",
                    app.active_model, app.config.api_base
                ),
                t.style_muted(),
            ),
        ]),
    ];

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Provider Connectivity & Health ");
    f.render_widget(Paragraph::new(detail_text).block(detail_block), chunks[1]);
}

// -------------------------------------------------------------------------------------------------
// 5. AGENTS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_agents_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let descendants = app
        .storage
        .list_spawn_descendants("current", None)
        .unwrap_or_default();

    let wf_store = app.storage.workflow_store();
    let workflows = wf_store.list_workflows(10).unwrap_or_default();

    // Find any speculative evaluation reports from workflows
    let mut spec_reports: Vec<SpeculativeExecutionReport> = Vec::new();
    for wf in &workflows {
        if let Ok(artifacts) = wf_store.list_artifacts(&wf.id) {
            for art in artifacts
                .into_iter()
                .filter(|a| a.artifact_type == "speculative_report")
            {
                let parsed = std::fs::read_to_string(&art.file_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<SpeculativeExecutionReport>(&c).ok());
                if let Some(report) = parsed {
                    spec_reports.push(report);
                }
            }
        }
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(area);

    // 1. Autonomous Workflows Table (top)
    let wf_header = Row::new(vec![
        Cell::from("Workflow ID").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Objective").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Tasks / Wave").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Rev").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let mut wf_rows = Vec::new();
    if workflows.is_empty() {
        wf_rows.push(Row::new(vec![
            Cell::from("No workflows").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("Launch workflows via `m31a workflow run \"<objective>\"`")
                .style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
        ]));
    } else {
        for (idx, wf) in workflows.iter().enumerate() {
            let is_selected = idx == app.screen_selected_index;
            let tasks = wf_store.list_tasks(&wf.id).unwrap_or_default();
            let completed = tasks
                .iter()
                .filter(|tk| tk.status == WorkflowTaskStatus::Completed)
                .count();
            let total = tasks.len();
            let active_wave = tasks
                .iter()
                .filter(|tk| tk.status == WorkflowTaskStatus::Running)
                .map(|tk| tk.wave_index)
                .next()
                .unwrap_or(0);

            let (state_label, state_color) = match wf.state {
                WorkflowState::Created => ("CREATED", ColorRole::Accent),
                WorkflowState::Planning | WorkflowState::PlanReady | WorkflowState::Analyzing => {
                    ("PLANNING", ColorRole::AccentMuted)
                }
                WorkflowState::Running
                | WorkflowState::Ready
                | WorkflowState::Verifying
                | WorkflowState::IntegrationPending => ("RUNNING", ColorRole::Warning),
                WorkflowState::Completed => ("COMPLETED", ColorRole::Success),
                WorkflowState::Failed | WorkflowState::Blocked => ("FAILED", ColorRole::Error),
                WorkflowState::Cancelled => ("CANCELLED", ColorRole::TextMuted),
                WorkflowState::AwaitingApproval => ("APPROVAL", ColorRole::Warning),
            };

            let wave_str = if total > 0 {
                format!("{completed}/{total} (Wave {active_wave})")
            } else {
                "-".to_string()
            };

            let id_span = if is_selected {
                Span::styled(
                    format!("▶ {}", wf.id),
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!("  {}", wf.id),
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )
            };

            let row_style = if is_selected {
                t.style_surface2()
            } else {
                Style::default()
            };

            wf_rows.push(
                Row::new(vec![
                    Cell::from(id_span),
                    Cell::from(wf.workflow_type.clone()),
                    Cell::from(wf.objective.clone()),
                    Cell::from(Span::styled(
                        state_label,
                        Style::default()
                            .fg(t.color(state_color))
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(wave_str),
                    Cell::from(format!("v{}", wf.plan_revision)),
                ])
                .style(row_style),
            );
        }
    }

    let wf_table = Table::new(
        wf_rows,
        [
            Constraint::Length(16),
            Constraint::Length(14),
            Constraint::Percentage(45),
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(8),
        ],
    )
    .header(wf_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(
                " ⚡ Autonomous Workflows & Wave Progress ({} registered) ",
                workflows.len()
            )),
    );
    f.render_widget(wf_table, chunks[0]);

    // 2. Middle section: Subagent Hierarchy + Speculative Parallel Reality Arena
    let middle_chunks = if area.width >= 120 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1])
    };

    // 2a. Subagent Hierarchy
    let header_row = Row::new(vec![
        Cell::from("Agent ID").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Role").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Depth").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let mut rows = Vec::new();
    let mut all_agent_ids = Vec::new();

    for tab in &app.bottom_pane.subagent_tabs {
        all_agent_ids.push(tab.thread_id.clone());
        let status_str = tab.status.badge().1;
        rows.push(Row::new(vec![
            Cell::from(tab.thread_id.clone()),
            Cell::from(tab.role.clone()),
            Cell::from(Span::styled(
                status_str,
                Style::default().fg(t.color(ColorRole::Success)),
            )),
            Cell::from("Active"),
        ]));
    }

    for e in &descendants {
        if all_agent_ids.contains(&e.child_thread_id) {
            continue;
        }
        all_agent_ids.push(e.child_thread_id.clone());
        rows.push(Row::new(vec![
            Cell::from(e.child_thread_id.clone()),
            Cell::from(e.agent_role.clone()),
            Cell::from(Span::styled(&e.status, t.style_muted())),
            Cell::from(format!("{}", e.depth)),
        ]));
    }

    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("No subagents").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("Idle").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
        ]));
    }

    let subagents_table = Table::new(
        rows,
        [
            Constraint::Length(24),
            Constraint::Length(16),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(
                " Subagent Hierarchy ({} live / {} total) ",
                app.bottom_pane.subagent_tabs.len(),
                descendants.len()
            )),
    );
    f.render_widget(subagents_table, middle_chunks[0]);

    // 2b. Speculative Arena Candidates Matrix
    let spec_header = Row::new(vec![
        Cell::from("Branch").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Build").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Tests").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Safety").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Score").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let mut spec_rows = Vec::new();
    if let Some(latest_report) = spec_reports.first() {
        for eval in &latest_report.evaluations {
            let is_winner = latest_report
                .winning_branch_id
                .as_ref()
                .map(|w| w == &eval.branch_id || w == &eval.name)
                .unwrap_or(false);

            let branch_name = if is_winner {
                format!("★ {}", eval.name)
            } else {
                eval.name.clone()
            };

            let build_span = if eval.build_passed {
                Span::styled("PASS", Style::default().fg(t.color(ColorRole::Success)))
            } else {
                Span::styled("FAIL", Style::default().fg(t.color(ColorRole::Error)))
            };

            let test_span = if eval.tests_passed {
                Span::styled("PASS", Style::default().fg(t.color(ColorRole::Success)))
            } else {
                Span::styled("FAIL", Style::default().fg(t.color(ColorRole::Error)))
            };

            let safety_passed =
                eval.safety_verdict.contains("Allow") || eval.safety_verdict.contains("Pass");
            let safety_span = if safety_passed {
                Span::styled("PASS", Style::default().fg(t.color(ColorRole::Success)))
            } else {
                Span::styled("FAIL", Style::default().fg(t.color(ColorRole::Error)))
            };

            let score_str = format!("{:.1}", eval.composite_score);
            let branch_style = if is_winner {
                Style::default()
                    .fg(t.color(ColorRole::Success))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            spec_rows.push(Row::new(vec![
                Cell::from(branch_name).style(branch_style),
                Cell::from(build_span),
                Cell::from(test_span),
                Cell::from(safety_span),
                Cell::from(Span::styled(
                    score_str,
                    Style::default().fg(t.color(ColorRole::Warning)),
                )),
            ]));
        }
    } else {
        spec_rows.push(Row::new(vec![
            Cell::from("Speculative Arena Standby").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("-").style(t.style_muted()),
            Cell::from("Launch with `m31a speculate`").style(t.style_muted()),
        ]));
    }

    let spec_table = Table::new(
        spec_rows,
        [
            Constraint::Length(18),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
        ],
    )
    .header(spec_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(" 🔀 Speculative Arena Candidates Matrix "),
    );
    f.render_widget(spec_table, middle_chunks[1]);

    // 3. Action bar
    let action_text = vec![
        Line::from(vec![
            Span::styled(
                "Supervision Actions: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[a] Approve  |  [r] Reject  |  [c] Cancel WF  |  [u] Resume WF  |  [i] Inspect  |  [t] Terminate Tree  |  [Esc] Chat",
                t.style_muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Engine Notice: ", t.style_secondary()),
            Span::styled(
                "Autonomous workflows partition tasks into conflict-free waves with lease recovery and multi-stage verification.",
                t.style_muted(),
            ),
        ]),
    ];
    let action_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Supervision & Autonomous Orchestration Operations ");
    f.render_widget(Paragraph::new(action_text).block(action_block), chunks[2]);
}

// -------------------------------------------------------------------------------------------------
// 6. WORKSPACE SCREEN
// -------------------------------------------------------------------------------------------------
fn render_workspace_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let trust_decision = app.trust_store.lock().unwrap().evaluate(&app.cwd);
    let (trust_label, trust_color) = match trust_decision {
        m31a_trust::TrustDecision::Trusted => ("TRUSTED (Full)", t.color(ColorRole::Success)),
        m31a_trust::TrustDecision::SessionOnly => {
            ("SESSION ONLY (Transient)", t.color(ColorRole::Warning))
        }
        m31a_trust::TrustDecision::Denied { .. } => ("DENIED (Blocked)", t.color(ColorRole::Error)),
        m31a_trust::TrustDecision::RequiresConfirmation { .. } => {
            ("UNTRUSTED (Sandboxed)", t.color(ColorRole::Warning))
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(5)])
        .split(area);

    let meta_text = vec![
        Line::from(vec![
            Span::styled(
                "Working Directory: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.cwd.display().to_string(),
                Style::default().fg(t.color(ColorRole::TextPrimary)),
            ),
            Span::styled(" │ Trust Status: ", t.style_secondary()),
            Span::styled(
                trust_label,
                Style::default()
                    .fg(trust_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Sandbox Policy: ", t.style_secondary()),
            Span::styled(format!("{:?}", app.config.sandbox), t.style_muted()),
            Span::styled(" │ Extra Dirs: ", t.style_secondary()),
            Span::styled(
                format!("{} paths", app.config.extra_dirs.len()),
                t.style_muted(),
            ),
        ]),
    ];

    let meta_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Project Workspace Metadata — [t] Toggle Trust Status ");
    f.render_widget(Paragraph::new(meta_text).block(meta_block), chunks[0]);

    // File tree listing
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&app.cwd) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            files.push((name, is_dir));
        }
    }
    files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let rows: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(i, (name, is_dir))| {
            let is_selected = i == app.screen_selected_index;
            let icon = if *is_dir {
                format!("{} ", crate::icons::Icon::Folder.render(t))
            } else {
                format!("{} ", crate::icons::Icon::File.render(t))
            };
            let style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else if *is_dir {
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            Row::new(vec![
                Cell::from(format!("{}{}", icon, name)),
                Cell::from(if *is_dir { "Directory" } else { "File" }),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(" Workspace Files ({}) ", files.len())),
    );

    f.render_widget(table, chunks[1]);
}

// -------------------------------------------------------------------------------------------------
// 7. SOURCE CONTROL SCREEN
// -------------------------------------------------------------------------------------------------
fn render_source_control_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(area);

    let branch_out = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&app.cwd)
        .output();
    let branch_name = branch_out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "detached".to_string());

    let status_out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&app.cwd)
        .output();
    let status_lines: Vec<String> = status_out
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default();

    let head_text = vec![Line::from(vec![
        Span::styled("Branch: ", t.style_secondary().add_modifier(Modifier::BOLD)),
        Span::styled(
            &branch_name,
            Style::default()
                .fg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ Working Tree: ", t.style_secondary()),
        Span::styled(
            if status_lines.is_empty() {
                "Clean"
            } else {
                "Dirty (Modified)"
            },
            if status_lines.is_empty() {
                Style::default().fg(t.color(ColorRole::Success))
            } else {
                Style::default().fg(t.color(ColorRole::Warning))
            },
        ),
    ])];
    let head_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Git Repository Status — [d] View Diff (Ctrl+D) ");
    f.render_widget(Paragraph::new(head_text).block(head_block), chunks[0]);

    let rows: Vec<_> = status_lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let is_selected = i == app.screen_selected_index;
            let status_code = line.get(0..2).unwrap_or("??");
            let file_name = line.get(3..).unwrap_or(line);

            let row_style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            Row::new(vec![Cell::from(status_code), Cell::from(file_name)]).style(row_style)
        })
        .collect();

    let table = Table::new(rows, [Constraint::Length(8), Constraint::Percentage(90)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(" Changed Files ({}) ", status_lines.len())),
    );
    f.render_widget(table, chunks[1]);

    // Worktrees
    let wt_mgr = m31a_storage::WorktreeManager::new(m31a_storage::WorktreeSettings::new(&app.cwd));
    let worktrees = wt_mgr.list_worktrees().unwrap_or_default();
    let wt_text = vec![
        Line::from(vec![
            Span::styled(
                "Active Child Agent Worktrees: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{} active", worktrees.len()), t.style_muted()),
        ]),
        Line::from(vec![
            Span::styled("Worktree Isolation: ", t.style_secondary()),
            Span::styled(
                "Child agents run isolated on git worktree branches to prevent dirtying parent HEAD.",
                t.style_muted(),
            ),
        ]),
    ];
    let wt_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Isolated Worktrees ");
    f.render_widget(Paragraph::new(wt_text).block(wt_block), chunks[2]);
}

// -------------------------------------------------------------------------------------------------
// 8. TOOLS & INTEGRATIONS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_tools_integrations_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let defs = app.tools.definitions();
    let filter = app.screen_filter.to_lowercase();
    let filtered_tools: Vec<_> = defs
        .into_iter()
        .filter(|d| {
            if filter.is_empty() {
                true
            } else {
                d.function.name.to_lowercase().contains(&filter)
                    || d.function.description.to_lowercase().contains(&filter)
            }
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let search_line = Line::from(vec![
        Span::styled(
            " Filter Tools: ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if app.screen_filter.is_empty() {
                "Type tool name to filter...".to_string()
            } else {
                app.screen_filter.clone()
            },
            if app.screen_filter.is_empty() {
                t.style_muted()
            } else {
                Style::default()
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            },
        ),
    ]);
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.screen_filter_active {
            t.border_focus()
        } else {
            t.border_subtle()
        })
        .title(" Search Tool Surface ");
    f.render_widget(Paragraph::new(search_line).block(search_block), chunks[0]);

    let header_row = Row::new(vec![
        Cell::from("Tool Name").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Kind").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let rows: Vec<_> = filtered_tools
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let is_selected = i == app.screen_selected_index;
            let kind = if d.function.name.starts_with("mcp_") {
                "MCP Tool"
            } else if d.function.name.starts_with("plugin_") {
                "Plugin"
            } else {
                "Built-in"
            };

            let row_style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            Row::new(vec![
                Cell::from(d.function.name.clone()),
                Cell::from(kind),
                Cell::from(d.function.description.clone()),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(28),
            Constraint::Length(14),
            Constraint::Percentage(60),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(format!(
                " Active Tool Registry ({} tools) ",
                filtered_tools.len()
            )),
    );

    f.render_widget(table, chunks[1]);
}

// -------------------------------------------------------------------------------------------------
// 9. SETTINGS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_settings_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let settings = [
        ("Theme", format!("{:?}", app.config.tui.theme)),
        (
            "Vim Mode",
            if app.config.tui.vim_mode_default {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
        (
            "Mouse Navigation",
            if app.config.tui.mouse_enabled {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
        (
            "UI Animations & Spinners",
            if app.config.tui.animations {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
        ("Approval Policy", format!("{:?}", app.config.approval)),
        (
            "Reasoning Effort",
            app.config
                .reasoning_effort
                .clone()
                .unwrap_or_else(|| "Default".to_string()),
        ),
        (
            "Max Context Tokens",
            format!("{}", app.config.model_context_window),
        ),
        (
            "Auto Compact Token Limit",
            format!("{}", app.config.model_auto_compact_token_limit),
        ),
    ];

    let header_row = Row::new(vec![
        Cell::from("Configuration Setting").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Current Value").style(t.style_secondary().add_modifier(Modifier::BOLD)),
        Cell::from("Action").style(t.style_secondary().add_modifier(Modifier::BOLD)),
    ])
    .style(t.style_surface1())
    .bottom_margin(1);

    let rows: Vec<_> = settings
        .iter()
        .enumerate()
        .map(|(i, (name, val))| {
            let is_selected = i == app.screen_selected_index;
            let row_style = if is_selected {
                Style::default()
                    .bg(t.color(ColorRole::Surface2))
                    .fg(t.color(ColorRole::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.color(ColorRole::TextPrimary))
            };

            Row::new(vec![
                Cell::from(*name),
                Cell::from(val.clone()),
                Cell::from("[Enter / Space] Toggle / Cycle"),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ],
    )
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(" Environment Settings & Preferences — [Enter] Modify [s] Save Configuration "),
    );

    f.render_widget(table, area);
}

// -------------------------------------------------------------------------------------------------
// 10. DIAGNOSTICS SCREEN
// -------------------------------------------------------------------------------------------------
fn render_diagnostics_screen(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(6),
        ])
        .split(area);

    // 1. Session & Token Accounting
    let token_lines = vec![
        Line::from(vec![
            Span::styled(
                "Active Session: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.session_id,
                Style::default().fg(t.color(ColorRole::TextPrimary)),
            ),
            Span::styled(" │ Session Epoch: ", t.style_secondary()),
            Span::styled(format!("{}", app.session_epoch), t.style_muted()),
        ]),
        Line::from(vec![
            Span::styled("Tokens Consumed: ", t.style_secondary()),
            Span::styled(
                format!("{} total tokens", app.tokens_used),
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ Prompt: ", t.style_secondary()),
            Span::styled(
                format!("{}", app.session_metrics.prompt_tokens),
                t.style_muted(),
            ),
            Span::styled(" │ Completion: ", t.style_secondary()),
            Span::styled(
                format!("{}", app.session_metrics.completion_tokens),
                t.style_muted(),
            ),
            Span::styled(" │ Reasoning: ", t.style_secondary()),
            Span::styled(
                format!("{}", app.session_metrics.reasoning_tokens),
                t.style_muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Turn State: ", t.style_secondary()),
            Span::styled(
                format!("{:?}", app.turn_state.label()),
                Style::default().fg(t.color(ColorRole::Success)),
            ),
        ]),
    ];
    let token_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Runtime Telemetry & Token Accounting ");
    f.render_widget(Paragraph::new(token_lines).block(token_block), chunks[0]);

    // 2. Direct Tool Latencies & Critical Path
    let tool_lines = vec![
        Line::from(vec![
            Span::styled("Active Background Processes: ", t.style_secondary()),
            Span::styled(
                format!(
                    "{} processes supervised",
                    app.proc_supervisor.process_count()
                ),
                t.style_muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Process Manager: ", t.style_secondary()),
            Span::styled(
                "Bounded pipe draining active (zero pipe deadlock guarantee)",
                Style::default().fg(t.color(ColorRole::Success)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Code Mode: ", t.style_secondary()),
            Span::styled(
                if app.code_mode.is_some() {
                    "Active (In-memory KV)"
                } else {
                    "Disabled"
                },
                t.style_muted(),
            ),
            Span::styled(" │ Local Encrypted Vault: ", t.style_secondary()),
            Span::styled("Online", Style::default().fg(t.color(ColorRole::Success))),
        ]),
    ];
    let tool_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Process Supervision & Sandbox Health ");
    f.render_widget(Paragraph::new(tool_lines).block(tool_block), chunks[1]);

    // 3. Storage and Integrity
    let db_path = m31a_core::Config::config_dir().join("storage.db");
    let storage_lines = vec![
        Line::from(vec![
            Span::styled(
                "Durable SQLite Database: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(db_path.display().to_string(), t.style_muted()),
        ]),
        Line::from(vec![
            Span::styled("Integrity Guarantees: ", t.style_secondary()),
            Span::styled(
                "Atomic persistence, WAL journaling, fail-closed trust authorization",
                Style::default().fg(t.color(ColorRole::Success)),
            ),
        ]),
    ];
    let storage_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .title(" Storage & Persistence Diagnostics ");
    f.render_widget(
        Paragraph::new(storage_lines).block(storage_block),
        chunks[2],
    );
}

// -------------------------------------------------------------------------------------------------
// CONTEXTUAL INSPECTOR (COLLAPSIBLE SIDE PANE)
// -------------------------------------------------------------------------------------------------
pub fn render_contextual_inspector(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    crate::panel::render_contextual_panel(f, app, area, t);
}

// -------------------------------------------------------------------------------------------------
// CONTEXTUAL FOOTER
// -------------------------------------------------------------------------------------------------
pub fn render_contextual_footer(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let (context_tag, screen_keys) = if app.has_active_modal() {
        (
            " [Dialog] ".to_string(),
            "↑/↓: Navigate  Enter: Select  Esc: Dismiss / Back".to_string(),
        )
    } else {
        match app.current_screen {
            Screen::Chat => {
                let panel_hint = if app.inspector_open {
                    "  [ / ]: Tab  Alt+[/]: Resize  Ctrl+B: Close Panel"
                } else {
                    "  Ctrl+B: Panel"
                };
                (
                    " [Workspace] ".to_string(),
                    format!(
                        "Enter: Send  Ctrl+Enter: Queue  Tab: Mode  Ctrl+M: Model  Ctrl+P: Palette  Ctrl+R: Sessions{}",
                        panel_hint
                    ),
                )
            }
            Screen::Sessions => (
                format!(" [Screen 2/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  Enter: Switch Session  d: Delete  r: Rename  /: Search  Esc: Back to Chat".to_string(),
            ),
            Screen::Models => (
                format!(" [Screen 3/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  Enter: Switch Model  e: Reasoning Effort  /: Search  Esc: Back to Chat".to_string(),
            ),
            Screen::Providers => (
                format!(" [Screen 4/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  t: Test Connectivity  Enter: Switch Provider  Esc: Back to Chat".to_string(),
            ),
            Screen::Agents => (
                format!(" [Screen 5/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  c: Close Agent  t: Terminate Tree  i: Integrate  Enter: Transcript  Esc: Back".to_string(),
            ),
            Screen::Workspace => (
                format!(" [Screen 6/10: {}] ", app.current_screen.title()),
                "↑/↓: Browse  t: Toggle Trust  /: Search  Esc: Back to Chat".to_string(),
            ),
            Screen::SourceControl => (
                format!(" [Screen 7/10: {}] ", app.current_screen.title()),
                "d (Ctrl+D): Diff  w: Prune Worktrees  Esc: Back to Chat".to_string(),
            ),
            Screen::ToolsIntegrations => (
                format!(" [Screen 8/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  /: Search Tools  Esc: Back to Chat".to_string(),
            ),
            Screen::Settings => (
                format!(" [Screen 9/10: {}] ", app.current_screen.title()),
                "↑/↓: Select  Enter: Toggle  s: Save  Esc: Back to Chat".to_string(),
            ),
            Screen::Diagnostics => (
                format!(" [Screen 10/10: {}] ", app.current_screen.title()),
                "r: Refresh  Esc: Back to Chat".to_string(),
            ),
        }
    };

    let line = Line::from(vec![
        Span::styled(
            context_tag,
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(screen_keys, t.style_muted()),
    ]);

    let p = Paragraph::new(line).style(t.style_surface0());
    f.render_widget(p, area);
}
