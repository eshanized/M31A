//! Universal Fuzzy Command Palette V2 (COP-03, D-05, T-13-12).
//!
//! Provides instant navigation across all 40 canonical views, active mission entities,
//! and runtime commands via fuzzy search (SkimMatcherV2) with credential masking.

use crossterm::event::{KeyCode, KeyEvent};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::cli::dispatch::RuntimeCommand;
use crate::tui::input::text_input::TextInput;
use crate::tui::registry::{ViewId, ViewRegistry};

/// Action triggered by selecting an item in the Universal Command Palette.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteActionV2 {
    NavigateView(ViewId),
    ExecuteCommand(RuntimeCommand),
    Action(String),
    Close,
}

/// A searchable command entry in the universal palette.
#[derive(Debug, Clone)]
pub struct PaletteItemV2 {
    pub id: String,
    pub category: &'static str,
    pub label: String,
    pub detail: Option<String>,
    pub shortcut: Option<String>,
    pub action: PaletteActionV2,
}

/// Universal Command Palette state and fuzzy search controller.
pub struct UniversalCommandPalette {
    items: Vec<PaletteItemV2>,
    matcher: SkimMatcherV2,
    query_input: TextInput,
    pub query: String,
    selected_index: usize,
    pub is_open: bool,
}

impl std::fmt::Debug for UniversalCommandPalette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UniversalCommandPalette")
            .field("items", &self.items)
            .field("query_input", &self.query_input)
            .field("query", &self.query)
            .field("selected_index", &self.selected_index)
            .field("is_open", &self.is_open)
            .finish()
    }
}

impl Default for UniversalCommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl UniversalCommandPalette {
    pub fn new() -> Self {
        let mut palette = Self {
            items: Vec::new(),
            matcher: SkimMatcherV2::default(),
            query_input: TextInput::single_line()
                .with_placeholder("Type a command or view name..."),
            query: String::new(),
            selected_index: 0,
            is_open: false,
        };
        palette.register_all_commands();
        palette
    }

    /// Populate all 40 canonical views, common runtime commands, and slash commands.
    pub fn register_all_commands(&mut self) {
        let registry = ViewRegistry::new();

        // 1. Register all 40 canonical views
        for meta in registry.all() {
            let shortcut_str = meta.hotkey.map(|c| c.to_string());
            let detail_str = if meta.id == ViewId::MissionDashboard {
                format!("{} Overview & Cockpit Metrics", meta.route_path)
            } else {
                meta.route_path.to_string()
            };
            self.items.push(PaletteItemV2 {
                id: format!("view_{}", meta.number),
                category: meta.domain_category,
                label: format!("[{:02}] {}", meta.number, meta.name),
                detail: Some(detail_str),
                shortcut: shortcut_str,
                action: PaletteActionV2::NavigateView(meta.id),
            });
        }

        // 2. Runtime operational actions
        self.items.push(PaletteItemV2 {
            id: "cmd_new_mission".to_string(),
            category: "Mission",
            label: "Create New Mission".to_string(),
            detail: Some("Open View 40 Mission Creation Composer".to_string()),
            shortcut: Some("N".to_string()),
            action: PaletteActionV2::NavigateView(ViewId::MissionCreation),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_pause_mission".to_string(),
            category: "Mission",
            label: "Mission: Pause Execution".to_string(),
            detail: Some("Signal runtime to suspend active agent turns".to_string()),
            shortcut: Some("Space".to_string()),
            action: PaletteActionV2::ExecuteCommand(RuntimeCommand::PauseMission {
                id: "current".to_string(),
            }),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_resume_mission".to_string(),
            category: "Mission",
            label: "Mission: Resume Execution".to_string(),
            detail: Some("Resume suspended execution stream".to_string()),
            shortcut: None,
            action: PaletteActionV2::ExecuteCommand(RuntimeCommand::ResumeMission {
                id: "current".to_string(),
            }),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_cancel_mission".to_string(),
            category: "Mission",
            label: "Mission: Emergency Cancel".to_string(),
            detail: Some("Emergency abort active mission execution immediately".to_string()),
            shortcut: None,
            action: PaletteActionV2::ExecuteCommand(RuntimeCommand::CancelMission {
                id: "current".to_string(),
                reason: Some("Cancelled from universal command palette".to_string()),
            }),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_doctor".to_string(),
            category: "Diagnostics",
            label: "Doctor: Run Diagnostics Probes".to_string(),
            detail: Some("Run 6-category health and configuration diagnostic probes".to_string()),
            shortcut: Some("0".to_string()),
            action: PaletteActionV2::ExecuteCommand(RuntimeCommand::RunDoctor {
                category: None,
                json: false,
            }),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_version".to_string(),
            category: "Diagnostics",
            label: "Version: Display M31A Engine Version".to_string(),
            detail: Some("Display engine build and version information".to_string()),
            shortcut: None,
            action: PaletteActionV2::ExecuteCommand(RuntimeCommand::Version { verbose: false }),
        });

        self.items.push(PaletteItemV2 {
            id: "cmd_toggle_theme".to_string(),
            category: "UI",
            label: "Cycle Color Theme".to_string(),
            detail: Some("Dark Slate Cyan -> High Contrast -> Clean Light -> ANSI".to_string()),
            shortcut: Some("F2".to_string()),
            action: PaletteActionV2::Action("toggle_theme".to_string()),
        });

        // 3. Register standard slash commands
        self.register_slash_commands(
            &crate::interaction::commands::SlashCommandRegistry::new_standard(),
        );

        // 4. Explicit governed lifecycle decisions. Each maps to a
        // real slash command handled by the canonical runtime coordinator; the
        // palette never invents governance actions.
        let governed: &[(&str, &str, &str, &str)] = &[
            (
                "gov_plan_accept",
                "Governance",
                "Plan: Accept candidate plan",
                "/plan accept",
            ),
            (
                "gov_plan_revise",
                "Governance",
                "Plan: Request model revision",
                "/plan revise",
            ),
            (
                "gov_plan_regen",
                "Governance",
                "Plan: Regenerate candidate plan",
                "/plan regen",
            ),
            (
                "gov_plan_reject",
                "Governance",
                "Plan: Reject candidate plan",
                "/plan reject",
            ),
            (
                "gov_tasks_accept",
                "Governance",
                "Tasks: Accept candidate task set",
                "/tasks accept",
            ),
            (
                "gov_tasks_regen",
                "Governance",
                "Tasks: Regenerate task graph",
                "/tasks regen",
            ),
            (
                "gov_authorize_yes",
                "Governance",
                "Authorize execution (workspace modification)",
                "/authorize yes",
            ),
            (
                "gov_authorize_no",
                "Governance",
                "Reject execution authorization",
                "/authorize no",
            ),
        ];
        for (id, category, label, cmd) in governed {
            self.items.push(PaletteItemV2 {
                id: (*id).to_string(),
                category,
                label: (*label).to_string(),
                detail: Some((*cmd).to_string()),
                shortcut: None,
                action: PaletteActionV2::Action((*cmd).to_string()),
            });
        }

        // 5. Governance inspectors: deterministic navigation to the contextual
        // detail surface backed by the authoritative revision projection.
        let inspectors: &[(&str, &str, &str, ViewId)] = &[
            (
                "gov_inspect_plan",
                "Governance",
                "Inspect plan under review",
                ViewId::DagInspector,
            ),
            (
                "gov_inspect_tasks",
                "Governance",
                "Inspect task graph under review",
                ViewId::TaskDetails,
            ),
            (
                "gov_inspect_auth",
                "Governance",
                "Inspect execution authorization",
                ViewId::ApprovalsQueue,
            ),
            (
                "gov_inspect_evidence",
                "Governance",
                "Inspect verification evidence",
                ViewId::VerificationSuite,
            ),
        ];
        for (id, category, label, view) in inspectors {
            self.items.push(PaletteItemV2 {
                id: (*id).to_string(),
                category,
                label: (*label).to_string(),
                detail: Some("Open contextual inspector".to_string()),
                shortcut: None,
                action: PaletteActionV2::NavigateView(*view),
            });
        }
    }

    /// Register slash commands from an authoritative command registry (incorporates user commands and aliases).
    pub fn register_slash_commands(
        &mut self,
        slash_registry: &crate::interaction::commands::SlashCommandRegistry,
    ) {
        self.items.retain(|item| item.category != "Slash Command");
        for cmd in slash_registry.commands() {
            self.items.push(PaletteItemV2 {
                id: format!("slash_{}", cmd.name),
                category: "Slash Command",
                label: format!("/{} - {}", cmd.name, cmd.description),
                detail: Some(cmd.usage.to_string()),
                shortcut: None,
                action: PaletteActionV2::Action(format!("/{}", cmd.name)),
            });
            for alias in &cmd.aliases {
                self.items.push(PaletteItemV2 {
                    id: format!("slash_alias_{}", alias),
                    category: "Slash Command",
                    label: format!("/{} (alias) - {}", alias, cmd.description),
                    detail: Some(format!("Alias for /{}", cmd.name)),
                    shortcut: None,
                    action: PaletteActionV2::Action(format!("/{}", alias)),
                });
            }
        }
    }

    /// Builder method to populate with an authoritative slash registry.
    pub fn with_slash_registry(
        mut self,
        slash_registry: &crate::interaction::commands::SlashCommandRegistry,
    ) -> Self {
        self.register_slash_commands(slash_registry);
        self
    }

    pub fn items(&self) -> &[PaletteItemV2] {
        &self.items
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.query_input.clear();
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query_input.clear();
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn query(&self) -> &str {
        if !self.query.is_empty() {
            &self.query
        } else {
            self.query_input.text()
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.query_input.set_text(query);
        self.selected_index = 0;
    }

    pub fn selected_item(&self) -> Option<&PaletteItemV2> {
        let filtered = self.filtered_items();
        filtered.get(self.selected_index).copied()
    }

    pub fn select_next(&mut self) {
        let count = self.filtered_items().len();
        if self.selected_index + 1 < count {
            self.selected_index += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn activate(&mut self) -> Option<PaletteActionV2> {
        let items = self.filtered_items();
        if let Some(item) = items.get(self.selected_index) {
            let action = item.action.clone();
            self.close();
            Some(action)
        } else {
            None
        }
    }

    /// Return filtered items matching current query, sorted by match score.
    pub fn filtered_items(&self) -> Vec<&PaletteItemV2> {
        let query_str = if !self.query.is_empty() {
            self.query.as_str()
        } else {
            self.query_input.text()
        };
        let query = query_str.trim();
        if query.is_empty() {
            return self.items.iter().collect();
        }

        let mut scored: Vec<(i64, &PaletteItemV2)> = self
            .items
            .iter()
            .filter_map(|item| {
                let target = format!(
                    "{} {} {}",
                    item.category,
                    item.label,
                    item.detail.as_deref().unwrap_or("")
                );
                self.matcher
                    .fuzzy_match(&target, query)
                    .map(|score| (score, item))
            })
            .collect();

        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.into_iter().map(|(_, item)| item).collect()
    }

    /// Process keyboard input.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PaletteActionV2> {
        match key.code {
            KeyCode::Esc => {
                self.close();
                Some(PaletteActionV2::Close)
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }
            KeyCode::Down => {
                let count = self.filtered_items().len();
                if self.selected_index + 1 < count {
                    self.selected_index += 1;
                }
                None
            }
            KeyCode::Enter => {
                let items = self.filtered_items();
                if let Some(item) = items.get(self.selected_index) {
                    let action = item.action.clone();
                    self.close();
                    Some(action)
                } else {
                    None
                }
            }
            _ => {
                if self.query_input.handle_key(key) {
                    self.query = self.query_input.text().to_string();
                    self.selected_index = 0;
                }
                None
            }
        }
    }

    /// Render command palette overlay centered in the terminal without model context.
    pub fn render(&self, f: &mut Frame, area: Rect) {
        self.render_with_model(f, area, None);
    }

    /// Render command palette overlay with model context for truthful command availability (P2, PAL-01).
    ///
    /// Clean application command search: quiet border, subtle selection,
    /// no neon accent.
    pub fn render_with_model(
        &self,
        f: &mut Frame,
        area: Rect,
        model: Option<&crate::tui::model::TuiViewModel>,
    ) {
        use crate::tui::theme::ThemeTokens;
        if !self.is_open {
            return;
        }

        let overlay_area = centered_rect(60, 55, area);
        f.render_widget(Clear, overlay_area);

        // Theme for palette chrome: prefer model-agnostic default tokens.
        let tokens = ThemeTokens::resolve(crate::tui::theme::ThemeMode::Default);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(tokens.separator);

        let inner = block.inner(overlay_area);
        f.render_widget(block, overlay_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(6),
                Constraint::Length(1),
            ])
            .split(inner);

        let display_text = if !self.query.is_empty() {
            self.query.clone()
        } else {
            self.query_input.display_text()
        };

        let query_p = Paragraph::new(Line::from(vec![
            Span::styled("  ", tokens.text_muted),
            Span::styled(display_text, tokens.text_primary),
        ]));
        f.render_widget(query_p, chunks[0]);

        let filtered = self.filtered_items();
        let mut lines = Vec::new();

        if filtered.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No matching commands",
                tokens.text_muted,
            )));
        } else {
            for (i, item) in filtered.iter().take(12).enumerate() {
                let is_sel = i == self.selected_index;
                let (is_available, disabled_reason) = check_command_availability(&item.id, model);

                let row_style = if is_sel {
                    tokens.selection.patch(tokens.text_primary)
                } else if is_available {
                    tokens.text_secondary
                } else {
                    tokens.text_muted
                };

                let mut spans = vec![Span::styled(format!("  {:<28}", item.label), row_style)];

                if let Some(reason) = disabled_reason {
                    spans.push(Span::styled(format!("  {}", reason), tokens.text_muted));
                } else if let Some(ref detail) = item.detail {
                    let short = if detail.len() > 40 {
                        format!("{}…", &detail[..40])
                    } else {
                        detail.clone()
                    };
                    spans.push(Span::styled(format!("  {short}"), tokens.text_muted));
                }

                lines.push(Line::from(spans));
            }
        }

        let results_p = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(results_p, chunks[1]);

        let footer_p = Paragraph::new(Line::from(Span::styled(
            "  ↑↓ navigate   Enter select   Esc close",
            tokens.text_muted,
        )))
        .alignment(Alignment::Center);

        f.render_widget(footer_p, chunks[2]);
    }
}

/// Check whether a command is currently available based on model projection (P2, PAL-01).
///
/// Returns `(is_available, disabled_reason)`.
pub fn check_command_availability(
    item_id: &str,
    model: Option<&crate::tui::model::TuiViewModel>,
) -> (bool, Option<&'static str>) {
    let Some(model) = model else {
        return (true, None);
    };

    match item_id {
        "cmd_pause_mission" => {
            let running = model.mission_status == "running"
                || model.mission_status == "executing"
                || model.session_status == "running";
            if running {
                (true, None)
            } else {
                (false, Some("No active running mission to pause"))
            }
        }
        "cmd_resume_mission" => {
            if model.mission_status == "paused" {
                (true, None)
            } else {
                (false, Some("Mission is not currently paused"))
            }
        }
        "cmd_cancel_mission" => {
            let active = model.mission_status == "running"
                || model.mission_status == "executing"
                || model.mission_status == "paused";
            if active {
                (true, None)
            } else {
                (false, Some("No active mission to cancel"))
            }
        }
        "gov_plan_accept" | "gov_plan_revise" | "gov_plan_regen" | "gov_plan_reject" => {
            if matches!(
                model.lifecycle.stage,
                crate::tui::lifecycle::TuiLifecycleStage::PlanReviewRequired
            ) {
                (true, None)
            } else {
                (false, Some("Requires PlanReviewRequired lifecycle stage"))
            }
        }
        "gov_tasks_accept" | "gov_tasks_regen" => {
            if matches!(
                model.lifecycle.stage,
                crate::tui::lifecycle::TuiLifecycleStage::TasksReviewRequired
            ) {
                (true, None)
            } else {
                (false, Some("Requires TasksReviewRequired lifecycle stage"))
            }
        }
        "gov_authorize_yes" | "gov_authorize_no" => {
            if matches!(
                model.lifecycle.stage,
                crate::tui::lifecycle::TuiLifecycleStage::ExecutionAuthorizationRequired
            ) {
                (true, None)
            } else {
                (
                    false,
                    Some("Requires ExecutionAuthorizationRequired lifecycle stage"),
                )
            }
        }
        _ => (true, None),
    }
}

/// Helper calculating centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
