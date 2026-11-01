//! Primary Ratatui frame renderer compositing headers, chat transcripts, modals, and composers.
//!
//! Draws the complete terminal screen layout on each tick or state modification.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;
use crate::modals::{
    ApprovalActionKind, ModalState, ScrubTurnItem, approval_action_rects, approval_action_shortcut,
    approval_inner_rect, approval_popup_rect,
};
use crate::theme::{ColorRole, Theme, theme};

pub fn render(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let t = theme();

    // Clear with base surface
    let base_block = Block::default().style(t.style_surface0());
    f.render_widget(base_block, size);

    let bottom_height = app.bottom_pane.desired_height(&app.composer);

    // Responsive layout calculations
    let is_compact_height = size.height < 24;
    let header_height = if is_compact_height { 2 } else { 3 };
    let show_panel = app.inspector_open && size.width >= 100;
    let is_chat = app.current_screen == crate::screens::Screen::Chat;

    // On Chat screen: the conversation is the primary workspace, so we omit the permanent 2-line navbar.
    // On other screens: navbar remains visible so the user can easily see and switch between the 10 destinations.
    let chunks = if is_chat {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Compact header
                Constraint::Min(5),                // Primary conversation workspace
                Constraint::Length(1),             // Contextual footer
            ])
            .margin(if size.height >= 35 { 1 } else { 0 })
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // Header
                Constraint::Length(2),             // 10-Screen Navbar
                Constraint::Min(5),                // Screen body
                Constraint::Length(1),             // Contextual footer
            ])
            .margin(if size.height >= 35 { 1 } else { 0 })
            .split(size)
    };

    render_header(f, app, chunks[0], t);

    let (workspace_area, footer_area) = if is_chat {
        (chunks[1], chunks[2])
    } else {
        crate::screens::render_navbar(f, app, chunks[1], t);
        (chunks[2], chunks[3])
    };

    // Split workspace horizontally if contextual panel is open and terminal is wide enough
    let (screen_area, panel_area_opt) = if show_panel {
        let panel_pct = app.context_panel_width_pct.clamp(20, 50);
        let main_pct = 100 - panel_pct;
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(main_pct),
                Constraint::Percentage(panel_pct),
            ])
            .split(workspace_area);
        (body_chunks[0], Some(body_chunks[1]))
    } else {
        (workspace_area, None)
    };

    if let Some(panel_area) = panel_area_opt {
        crate::panel::render_contextual_panel(f, app, panel_area, t);
    }

    if is_chat {
        let chat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),                // Transcript
                Constraint::Length(bottom_height), // Bottom pane
            ])
            .split(screen_area);

        // Synchronize virtualizer line dimensions with actual viewport
        let transcript_inner = Block::default().borders(Borders::ALL).inner(chat_chunks[0]);
        if transcript_inner.width > 0 && transcript_inner.height > 0 {
            let total_lines = app
                .transcript_virtualizer
                .total_line_count(transcript_inner.width);
            app.chat_widget
                .scroll_controller
                .update_dimensions(total_lines, transcript_inner.height as usize);
            app.auto_scroll = app.chat_widget.scroll_controller.is_pinned_to_bottom;
            app.scroll_offset = app.chat_widget.scroll_controller.offset();
        }

        if let crate::bottom_pane::BottomPaneMode::ModelSelectorPopup { .. } = app.bottom_pane.mode
        {
            let popup_area =
                crate::bottom_pane::BottomPaneState::model_selector_popup_area(chat_chunks[1]);
            let inner_height = popup_area.height.saturating_sub(2) as usize;
            if let Some(ref mut ms) = app.bottom_pane.model_selector {
                ms.set_viewport_height(inner_height);
            }
        }

        render_transcript(f, app, chat_chunks[0], t);
        app.bottom_pane.render(f, chat_chunks[1], t, &app.composer);
    } else {
        if app.current_screen == crate::screens::Screen::Models {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(5)])
                .split(screen_area);
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(chunks[1]);
            let inner_h = body_chunks[0].height.saturating_sub(2) as usize;
            if let Some(ref mut ms) = app.models_screen_state {
                ms.set_viewport_height(inner_h);
            }
        }
        crate::screens::render_screen_content(f, app, screen_area, t);
    }

    crate::screens::render_contextual_footer(f, app, footer_area, t);

    // Modal overlays backed by canonical ModalStack
    if let Some(ModalState::ModelPicker { state }) = app.current_modal_mut() {
        let popup_area = centered_rect(72, 70, size);
        let inner_h = popup_area.height.saturating_sub(2) as usize;
        state.set_viewport_height(inner_h);
    }
    if let Some(modal) = app.current_modal() {
        render_modal(f, modal, app, size, t);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let (badge_text, badge_color) = app.chat_widget.format_status_badge();
    let status_style = Style::default()
        .fg(badge_color)
        .add_modifier(Modifier::BOLD);

    let project_name = app
        .cwd
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| app.cwd.to_string_lossy().to_string());

    let (mode_badge, mode_bg, mode_fg) = match app.collaboration_mode {
        m31a_core::prompts::CollaborationMode::Default => (
            " BUILD ",
            t.color(ColorRole::Success),
            t.color(ColorRole::Surface0),
        ),
        m31a_core::prompts::CollaborationMode::Plan => (
            " PLAN ",
            t.color(ColorRole::Warning),
            t.color(ColorRole::Surface0),
        ),
    };

    let model_display = if let Some(effort) = app.nim_client.reasoning_effort() {
        format!("{} [{}]", app.active_model, effort)
    } else {
        app.active_model.clone()
    };

    let session_short = if app.session_id.len() > 8 {
        &app.session_id[..8]
    } else {
        &app.session_id
    };

    let title_line = Line::from(vec![
        Span::styled(
            " M31A ",
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            mode_badge,
            Style::default()
                .fg(mode_fg)
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            model_display,
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", t.style_muted()),
        Span::styled(
            format!("{} {}", crate::icons::Icon::Folder.render(t), project_name),
            t.style_primary(),
        ),
        Span::styled(" │ ", t.style_muted()),
        Span::styled(
            format!(
                "{} #{}",
                crate::icons::Icon::MessageSquare.render(t),
                session_short
            ),
            t.style_muted(),
        ),
        Span::styled(" │ ", t.style_muted()),
        Span::styled(format!("● {}", badge_text), status_style),
    ]);

    let shortcuts_hint = if app.inspector_open {
        "[Ctrl+P: Commands] [Ctrl+B: Panel ▾]"
    } else {
        "[Ctrl+P: Commands] [Ctrl+B: Panel ▹]"
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.default)
        .border_style(t.border_default())
        .style(t.style_surface1());

    let header_p = Paragraph::new(title_line).block(block);
    f.render_widget(header_p, area);

    let hint_len = shortcuts_hint.len() as u16;
    if area.width > hint_len + 55 && area.height >= 2 {
        let hint_area = Rect {
            x: area.x + area.width.saturating_sub(hint_len + 2),
            y: area.y + 1,
            width: hint_len,
            height: 1,
        };
        let hint_p = Paragraph::new(Span::styled(shortcuts_hint, t.style_muted()));
        f.render_widget(hint_p, hint_area);
    }
}

fn render_transcript(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.default)
        .title(Line::from(vec![Span::styled(
            " Conversation ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_default())
        .style(t.style_surface1());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height > 0 && inner.width > 0 {
        // Render mouse selection highlight behind text if dragging
        if let Some(sel) = app.mouse_selection
            && sel.active
        {
            let min_row = sel.start.1.min(sel.current.1);
            let max_row = sel.start.1.max(sel.current.1);
            if max_row >= inner.y && min_row < inner.y + inner.height {
                let clamped_min = min_row.max(inner.y);
                let clamped_max = max_row.min(inner.y + inner.height.saturating_sub(1));
                let sel_area = Rect::new(
                    inner.x,
                    clamped_min,
                    inner.width,
                    clamped_max - clamped_min + 1,
                );
                let sel_block =
                    Block::default().style(Style::default().bg(t.color(ColorRole::Surface2)));
                f.render_widget(sel_block, sel_area);
            }
        }

        app.transcript_virtualizer.render_virtualized(
            f,
            inner,
            app.chat_widget.scroll_controller.offset(),
        );

        // Vertical scrollbar
        let total_lines = app.transcript_virtualizer.total_line_count(inner.width);
        let offset = app.chat_widget.scroll_controller.offset();
        let viewport_h = inner.height as usize;

        if total_lines > viewport_h {
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let mut scrollbar_state =
                ScrollbarState::new(total_lines.saturating_sub(viewport_h)).position(offset);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█")
                .style(t.style_muted());
            f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }

        // Unpinned viewport indicator
        if total_lines > viewport_h && offset + viewport_h < total_lines {
            let lines_below = total_lines.saturating_sub(offset + viewport_h);
            let indicator_text = format!(" ↓ {} lines below — End to jump ", lines_below);
            let pill_width = (indicator_text.len() as u16).min(inner.width.saturating_sub(4));
            let pill_x = inner.x + inner.width.saturating_sub(pill_width + 1);
            let pill_y = inner.y + inner.height.saturating_sub(1);
            let pill_rect = Rect::new(pill_x, pill_y, pill_width, 1);

            let pill = Paragraph::new(Line::from(vec![Span::styled(
                indicator_text,
                Style::default()
                    .fg(t.color(ColorRole::Surface0))
                    .bg(t.color(ColorRole::Warning))
                    .add_modifier(Modifier::BOLD),
            )]));
            f.render_widget(pill, pill_rect);
        }
    }
}

fn render_modal(f: &mut Frame, modal: &ModalState, app: &App, area: Rect, t: &Theme) {
    match modal {
        ModalState::AnalyticsDashboard => {
            render_analytics_dashboard(f, &app.session_metrics, area, t);
        }
        ModalState::ExperimentalFeatures {
            items,
            selected_index,
        } => {
            render_experimental_features(f, items, *selected_index, area, t);
        }
        ModalState::InteractiveDiff(state) => {
            render_interactive_diff(f, state, area, t);
        }
        ModalState::QuestionPrompt(q) => {
            render_question_prompt(f, q, area, t);
        }
        ModalState::ModelPicker { state } => {
            render_model_picker(f, state, area, t);
        }
        ModalState::ApprovalDialog {
            approval,
            selected_option,
        } => {
            render_approval_dialog(f, approval, *selected_option, area, t);
        }
        ModalState::DeclineFeedbackInput {
            approval,
            feedback_buffer,
        } => {
            render_decline_feedback_modal(f, approval, feedback_buffer, area, t);
        }
        ModalState::QuickDiffPreview {
            diff_text, scroll, ..
        } => {
            render_quick_diff_preview(f, diff_text, *scroll, area, t);
        }
        ModalState::ResumePicker(picker) => {
            let popup_area = centered_rect(80, 75, area);
            picker.render(f, popup_area);
        }
        ModalState::DiffViewer { diff_text, scroll } => {
            render_diff_viewer(f, diff_text, *scroll, area, t);
        }
        ModalState::SymbolOutlineModal {
            file_path,
            symbols,
            selected_index,
        } => {
            render_symbol_outline(f, file_path, symbols, *selected_index, area, t);
        }
        ModalState::CriticalPathModal {
            durations,
            completed_intervals,
            scroll,
        } => {
            render_critical_path_modal(f, durations, completed_intervals, *scroll, area, t);
        }
        ModalState::ActiveLocksModal {
            locks,
            selected_index,
        } => {
            render_active_locks_modal(f, locks, *selected_index, area, t);
        }
        ModalState::CheckpointsModal {
            checkpoints,
            selected_index,
        } => {
            render_checkpoints_modal(f, checkpoints, *selected_index, area, t);
        }
        ModalState::RevertConfirmModal {
            checkpoint,
            selected_option,
        } => {
            render_revert_confirm_modal(f, checkpoint, *selected_option, area, t);
        }
        ModalState::ProcessesModal {
            processes,
            selected_index,
        } => {
            render_processes_modal(f, processes, *selected_index, area, t);
        }
        ModalState::ProcessViewer {
            process_id,
            label,
            lines,
            scroll,
        } => {
            render_process_viewer(f, process_id, label, lines, *scroll, area, t);
        }
        ModalState::HelpModal { scroll } => {
            render_help_modal(f, *scroll, area, t);
        }
        ModalState::CommandPalette {
            query,
            selected_index,
            scroll_offset,
            items,
            filtered_indices,
        } => {
            render_command_palette(
                f,
                query,
                *selected_index,
                *scroll_offset,
                items,
                filtered_indices,
                area,
                t,
            );
        }
        ModalState::SettingsModal {
            active_tab,
            selected_index,
        } => {
            render_settings_modal(f, app, *active_tab, *selected_index, area, t);
        }
        ModalState::WorktreeSelector {
            worktrees,
            selected_index,
        } => {
            render_worktree_selector_modal(f, worktrees, *selected_index, &app.cwd, area, t);
        }
        ModalState::McpHubModal {
            servers,
            selected_index,
            view_tools,
        } => {
            render_mcp_hub_modal(f, servers, *selected_index, *view_tools, area, t);
        }
        ModalState::SkillsModal {
            skills,
            selected_index,
            search_query,
        } => {
            render_skills_modal(f, skills, *selected_index, search_query, area, t);
        }
        ModalState::TemplatesModal {
            templates,
            selected_index,
        } => {
            render_templates_modal(f, templates, *selected_index, area, t);
        }
        ModalState::GoalDashboardModal { goal, scroll } => {
            render_goal_dashboard_modal(f, goal, *scroll, area, t);
        }
        ModalState::SubagentsModal {
            subagents,
            selected_index,
            recent_posts,
        } => {
            render_subagents_modal(f, subagents, recent_posts, *selected_index, area, t);
        }
        ModalState::TurnScrubber {
            turns,
            selected_index,
        } => {
            render_turn_scrubber_modal(f, turns, *selected_index, area, t);
        }
        ModalState::ContextManagerModal {
            pinned_items,
            world_sections,
            selected_index,
            active_tab,
        } => {
            render_context_manager_modal(
                f,
                app,
                pinned_items,
                world_sections,
                *selected_index,
                *active_tab,
                area,
                t,
            );
        }
        ModalState::PersonaModal {
            personas,
            selected_index,
        } => {
            render_persona_modal(f, personas, *selected_index, area, t);
        }
        ModalState::CodeModeScratchpad(sp) => {
            render_code_mode_scratchpad(f, sp, area, t);
        }
        ModalState::RolloutJournal(journal) => {
            render_rollout_journal(f, journal, area, t);
        }
        ModalState::SelectPicker(picker) => {
            render_select_picker_modal(f, picker, area, t);
        }
        ModalState::SetupWizard { state } => {
            render_setup_wizard(f, state, area, t);
        }
        ModalState::ReasoningEffort { state } => {
            render_reasoning_effort(f, state, area, t);
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

// ===== Modal Renderers =====

pub fn model_picker_popup_rect(area: Rect) -> Rect {
    let percent_x = if area.width < 90 {
        92
    } else if area.width < 120 {
        88
    } else {
        80
    };
    let percent_y = if area.height < 30 { 85 } else { 75 };
    centered_rect(percent_x, percent_y, area)
}

fn render_model_picker(
    f: &mut Frame,
    state: &crate::model_selector::ModelListState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = model_picker_popup_rect(area);
    f.render_widget(Clear, popup_area);

    crate::model_selector::render_model_list(f, state, popup_area, t, "Select Model");
}

fn render_approval_dialog(
    f: &mut Frame,
    approval: &crate::modals::PendingApproval,
    selected_option: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = approval_popup_rect(area);
    f.render_widget(Clear, popup_area);
    let inner = approval_inner_rect(popup_area);
    let action_rows = approval_action_rects(area, approval);
    let kind_count = action_rows.len();
    let action_start = action_rows
        .first()
        .map(|(rect, _)| rect.y)
        .unwrap_or(inner.y + inner.height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                format!(
                    " {} Approval Required ",
                    crate::icons::Icon::ShieldAlert.render(t)
                ),
                t.style_error(true),
            ),
            Span::styled(" [Single-Keystroke Actions] ", t.style_muted()),
        ]))
        .border_style(t.style_error(false))
        .style(t.style_surface1());

    let mut body = vec![
        Line::from(vec![
            Span::styled("Tool: ", t.style_secondary().add_modifier(Modifier::BOLD)),
            Span::styled(
                &approval.tool_call.function.name,
                t.style_primary().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Reason: ", t.style_warning(true)),
            Span::styled(&approval.reason, t.style_primary()),
        ]),
    ];

    if let Some(cmd) = &approval.command {
        body.push(Line::raw(""));
        body.push(Line::from(vec![Span::styled(
            "Command to Execute:",
            t.style_info(true),
        )]));
        for line in cmd.lines() {
            body.push(Line::from(vec![
                Span::raw("  $ "),
                Span::styled(line.to_string(), t.style_warning(true)),
            ]));
        }
    } else if let Some(target) = approval.target_file() {
        body.push(Line::raw(""));
        body.push(Line::from(vec![
            Span::styled("Target File: ", t.style_info(true)),
            Span::styled(target, t.style_accent(false)),
        ]));
    }

    if let Some(diff) = &approval.diff {
        body.push(Line::from(vec![
            Span::styled("Diff: ", t.style_info(true)),
            Span::styled(
                format!("{} lines available (press v)", diff.lines().count()),
                t.style_primary(),
            ),
        ]));
    }

    // Reserve two rows for the separator above the action block.
    let body_capacity = action_start.saturating_sub(inner.y).saturating_sub(2) as usize;
    if body.len() > body_capacity {
        body.truncate(body_capacity.saturating_sub(1));
        body.push(Line::styled(
            "… output truncated; press v to inspect the full diff",
            t.style_muted(),
        ));
    }
    while body.len() < body_capacity {
        body.push(Line::raw(""));
    }

    let selected = selected_option.min(kind_count.saturating_sub(1));
    let mut action_lines = Vec::with_capacity(kind_count + 2);
    action_lines.push(Line::styled(
        "── Actions ────────────────────────────────────────────────",
        t.style_muted(),
    ));
    action_lines.push(Line::raw(""));
    for (index, (_, kind)) in action_rows.iter().enumerate() {
        let is_selected = index == selected;
        let (bg, fg) = match kind {
            ApprovalActionKind::AllowOnce => (ColorRole::Success, ColorRole::Surface0),
            ApprovalActionKind::AllowRule => (ColorRole::Accent, ColorRole::Surface0),
            ApprovalActionKind::AllowFile => (ColorRole::Warning, ColorRole::Surface0),
            ApprovalActionKind::ViewDiff => (ColorRole::Info, ColorRole::Surface0),
            ApprovalActionKind::DeclineWithFeedback => (ColorRole::Error, ColorRole::Surface0),
            ApprovalActionKind::Cancel => (ColorRole::Surface2, ColorRole::TextPrimary),
        };
        let style = if is_selected {
            Style::default()
                .fg(t.color(fg))
                .bg(t.color(bg))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(bg))
                .add_modifier(Modifier::BOLD)
        };
        let label = match kind {
            ApprovalActionKind::AllowOnce => "Allow Once".to_string(),
            ApprovalActionKind::AllowRule => format!(
                "Allow & Amend Rule (prefix: '{}')",
                approval.suggested_rule.as_deref().unwrap_or("")
            ),
            ApprovalActionKind::AllowFile => format!(
                "Allow File This Session ('{}')",
                approval.allow_file.as_deref().unwrap_or("")
            ),
            ApprovalActionKind::ViewDiff => {
                "View Diff Fullscreen (inspect hunks before approving)".to_string()
            }
            ApprovalActionKind::DeclineWithFeedback => "Decline with Feedback".to_string(),
            ApprovalActionKind::Cancel => "Cancel / Deny without note".to_string(),
        };
        action_lines.push(Line::from(vec![
            Span::styled(format!(" [{}] ", approval_action_shortcut(*kind)), style),
            Span::styled(label, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
    }

    let mut lines = body;
    lines.extend(action_lines);
    lines.truncate(inner.height as usize);
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_decline_feedback_modal(
    f: &mut Frame,
    approval: &crate::modals::PendingApproval,
    feedback: &str,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(65, 35, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            format!(
                " {} Decline with Feedback ",
                crate::icons::Icon::CircleX.render(t)
            ),
            t.style_error(true),
        )]))
        .border_style(t.style_error(false))
        .style(t.style_surface1());

    let tool_name = &approval.tool_call.function.name;
    let target = approval.command.as_deref().unwrap_or(tool_name.as_str());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Declining: ", t.style_secondary()),
            Span::styled(target, t.style_warning(true)),
        ]),
        Line::raw(""),
        Line::styled(
            "Guide the agent on what to do instead (or leave empty):",
            t.style_primary(),
        ),
        Line::raw(""),
    ];

    // Input box simulation with cursor
    let input_line = Line::from(vec![
        Span::styled(" > ", t.style_accent(true)),
        Span::styled(feedback, t.style_primary().add_modifier(Modifier::BOLD)),
        Span::styled("█", t.style_accent(true)),
    ]);
    lines.push(input_line);

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(
            " [Enter] ",
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Error))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Submit Feedback & Deny    ", t.style_primary()),
        Span::styled(" [Esc] ", t.style_muted().add_modifier(Modifier::BOLD)),
        Span::styled(" Back to Approval", t.style_muted()),
    ]));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup_area);
}

fn render_quick_diff_preview(
    f: &mut Frame,
    diff_text: &[String],
    scroll: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(88, 85, area);
    f.render_widget(Clear, popup_area);

    let joined = diff_text.join("\n");
    let parsed = crate::diff_render::DiffRenderer::parse_unified_diff(&joined);
    let stats = crate::diff_render::DiffRenderer::summarize(&parsed);
    let lines = crate::diff_render::DiffRenderer::render_lines(&parsed, t);

    let title = format!(
        " {} Quick Diff Preview  +{} −{}  [Esc: Return to Approval]  [y: Allow]  [n: Decline] ",
        crate::icons::Icon::Search.render(t),
        stats.additions,
        stats.deletions
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_info(true).add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)));

    let p = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    f.render_widget(p, popup_area);
}

fn render_diff_viewer(f: &mut Frame, diff_text: &[String], scroll: usize, area: Rect, t: &Theme) {
    let popup_area = centered_rect(88, 85, area);
    f.render_widget(Clear, popup_area);

    let joined = diff_text.join("\n");
    let parsed = crate::diff_render::DiffRenderer::parse_unified_diff(&joined);
    let stats = crate::diff_render::DiffRenderer::summarize(&parsed);
    let lines = crate::diff_render::DiffRenderer::render_lines(&parsed, t);
    let total_lines = lines.len();

    let title = format!(
        " Git Diff  +{} −{}  {} hunks  {} files  ↑↓/Wheel=Scroll Esc=Close ",
        stats.additions, stats.deletions, stats.hunks, stats.files
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Success)))
        .style(t.style_surface1());

    let p = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    f.render_widget(p, popup_area);

    let visible_h = popup_area.height.saturating_sub(2) as usize;
    if total_lines > visible_h {
        use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
        let mut scrollbar_state =
            ScrollbarState::new(total_lines.saturating_sub(visible_h)).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(t.style_muted());
        f.render_stateful_widget(scrollbar, popup_area, &mut scrollbar_state);
    }
}

fn render_symbol_outline(
    f: &mut Frame,
    file_path: &str,
    symbols: &[m31a_core::SymbolItem],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 80, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Symbol Outline: {}  ({} symbols)  ↑↓ Navigate Enter=Inspect Esc=Close ",
        file_path,
        symbols.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)))
        .style(t.style_surface1());

    let items: Vec<ListItem> = symbols
        .iter()
        .enumerate()
        .map(|(i, sym)| {
            let is_sel = i == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            let kind_badge = format!("[{:?}]", sym.kind);
            let badge_color = match sym.kind {
                m31a_core::SymbolKind::Function | m31a_core::SymbolKind::Method => {
                    t.color(ColorRole::Info)
                }
                m31a_core::SymbolKind::Struct | m31a_core::SymbolKind::Class => {
                    t.color(ColorRole::Success)
                }
                m31a_core::SymbolKind::Trait | m31a_core::SymbolKind::Interface => {
                    t.color(ColorRole::Accent)
                }
                m31a_core::SymbolKind::Enum => t.color(ColorRole::Warning),
                m31a_core::SymbolKind::TypeAlias => t.color(ColorRole::Info),
                m31a_core::SymbolKind::Constant => t.color(ColorRole::Error),
            };

            let line = Line::from(vec![
                Span::styled(if is_sel { "▸ " } else { "  " }, row_style),
                Span::styled(
                    format!("{: <10} ", kind_badge),
                    Style::default().fg(badge_color),
                ),
                Span::styled(format!("L{: <4} ", sym.line), t.style_muted()),
                Span::styled(&sym.name, row_style.add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(&sym.signature, t.style_muted()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, popup_area);
}

fn render_help_modal(f: &mut Frame, scroll: usize, area: Rect, t: &Theme) {
    let popup_area = centered_rect(75, 75, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " Help & Keybindings ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Esc=Close ", t.style_muted()),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let mut lines = vec![
        Line::from(Span::styled(
            "M31A — Autonomous Agentic Coding Assistant",
            t.style_accent(true),
        )),
        Line::from(Span::styled(
            "Powered by NVIDIA NIM frontier models",
            t.style_muted(),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "KEYBOARD SHORTCUTS",
            t.style_secondary()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
    ];

    for kb in crate::keybindings::CANONICAL_KEYBINDINGS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18} ", kb.key), t.style_accent(true)),
            Span::raw(kb.description),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "INTERACTIVE CONTROLS",
        t.style_secondary()
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    lines.push(Line::from(vec![
        Span::styled("  @ (in composer)    ", t.style_accent(true)),
        Span::raw("File/symbol mention autocomplete"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  / (in composer)    ", t.style_accent(true)),
        Span::raw("Slash command autocomplete"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  !cmd               ", t.style_accent(true)),
        Span::raw("Instant shell execution"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Mouse Wheel        ", t.style_accent(true)),
        Span::raw("Scroll transcript & modals with acceleration"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Mouse Click        ", t.style_accent(true)),
        Span::raw("Select list items, toggle settings, dismiss modals"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Mouse Drag         ", t.style_accent(true)),
        Span::raw("Select transcript text (auto-copies to clipboard)"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Scrollbar Track    ", t.style_accent(true)),
        Span::raw("Click arrows or drag scrollbar track to jump"),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "SLASH COMMANDS",
        t.style_secondary()
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    for cmd in m31a_composer::slash::CANONICAL_SLASH_COMMANDS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18} ", cmd.usage), t.style_success(true)),
            Span::raw(cmd.description),
        ]));
    }

    let total_lines = lines.len();
    let p = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    f.render_widget(p, popup_area);

    let visible_h = popup_area.height.saturating_sub(2) as usize;
    if total_lines > visible_h {
        use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
        let mut scrollbar_state =
            ScrollbarState::new(total_lines.saturating_sub(visible_h)).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(t.style_muted());
        f.render_stateful_widget(scrollbar, popup_area, &mut scrollbar_state);
    }
}

fn render_critical_path_modal(
    f: &mut Frame,
    durations: &m31a_core::CausalPathDurations,
    completed_intervals: &[m31a_core::CompletedWork],
    scroll: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 80, area);
    f.render_widget(Clear, popup_area);

    let total_ms = durations.total().as_millis().max(1);
    let title = format!(
        " Turn Timing Breakdown  Total: {}ms  ↑↓ Scroll Esc=Close ",
        total_ms
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Warning)))
        .style(t.style_surface1());

    let inf_ms = durations.inference.as_millis();
    let tools_ms = durations.tools.as_millis();
    let rev_ms = durations.auto_review.as_millis();
    let wait_ms = durations.user_wait.as_millis();
    let over_ms = durations.overhead.as_millis();

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            "Category Attribution: ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("Inference {}ms ({}%) ", inf_ms, (inf_ms * 100) / total_ms),
            t.style_info(false),
        ),
        Span::styled(
            format!("Tools {}ms ({}%) ", tools_ms, (tools_ms * 100) / total_ms),
            t.style_success(false),
        ),
        Span::styled(
            format!("Review {}ms ({}%) ", rev_ms, (rev_ms * 100) / total_ms),
            t.style_warning(false),
        ),
        Span::styled(
            format!("UserWait {}ms ({}%) ", wait_ms, (wait_ms * 100) / total_ms),
            t.style_muted(),
        ),
        Span::styled(format!("Overhead {}ms", over_ms), t.style_muted()),
    ]));

    // Visual proportional bar
    let bar_width = 60usize;
    let inf_chars = ((inf_ms as f64 / total_ms as f64) * bar_width as f64).round() as usize;
    let tools_chars = ((tools_ms as f64 / total_ms as f64) * bar_width as f64).round() as usize;
    let rev_chars = ((rev_ms as f64 / total_ms as f64) * bar_width as f64).round() as usize;
    let wait_chars = ((wait_ms as f64 / total_ms as f64) * bar_width as f64).round() as usize;
    let over_chars = bar_width.saturating_sub(inf_chars + tools_chars + rev_chars + wait_chars);

    lines.push(Line::from(vec![
        Span::raw("["),
        Span::styled("█".repeat(inf_chars), t.style_info(false)),
        Span::styled("▓".repeat(tools_chars), t.style_success(false)),
        Span::styled("▒".repeat(rev_chars), t.style_warning(false)),
        Span::styled("░".repeat(wait_chars), t.style_muted()),
        Span::styled("·".repeat(over_chars), t.style_muted()),
        Span::raw("]"),
    ]));
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled(
        "Completed Work Intervals:",
        t.style_secondary()
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    if completed_intervals.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No direct tool invocations recorded)",
            t.style_muted(),
        )));
    } else {
        for (idx, interval) in completed_intervals.iter().enumerate().skip(scroll) {
            let status_badge = if interval.is_error {
                Span::styled("[ERR]", t.style_error(true))
            } else {
                Span::styled("[OK]", t.style_success(true))
            };

            lines.push(Line::from(vec![
                Span::raw(format!(" {:2}. ", idx + 1)),
                status_badge,
                Span::raw(" "),
                Span::styled(
                    &interval.name,
                    t.style_primary().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" ({:?}) ", interval.kind)),
                Span::styled(
                    format!("{}ms", interval.duration.as_millis()),
                    t.style_info(false),
                ),
            ]));
        }
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);

    let visible_h = popup_area.height.saturating_sub(6) as usize;
    if completed_intervals.len() > visible_h {
        use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
        let mut scrollbar_state =
            ScrollbarState::new(completed_intervals.len().saturating_sub(visible_h))
                .position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(t.style_muted());
        f.render_stateful_widget(scrollbar, popup_area, &mut scrollbar_state);
    }
}

fn render_active_locks_modal(
    f: &mut Frame,
    locks: &[m31a_core::LockEntry],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 75, area);
    f.render_widget(Clear, popup_area);

    let write_count = locks
        .iter()
        .filter(|l| matches!(l.mode, m31a_core::LockMode::Write))
        .count();
    let read_count = locks
        .iter()
        .filter(|l| matches!(l.mode, m31a_core::LockMode::Read))
        .count();

    let title = format!(
        " Concurrency Locks  {} Active: {} Write {} Read  ↑↓ Navigate Esc=Close ",
        locks.len(),
        write_count,
        read_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Warning)))
        .style(t.style_surface1());

    let mut lines = Vec::new();
    if locks.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No active concurrency locks",
            t.style_muted(),
        )));
    } else {
        for (idx, entry) in locks.iter().enumerate() {
            let is_sel = idx == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let marker = if is_sel { "▸ " } else { "  " };

            let (mode_badge, mode_color) = match entry.mode {
                m31a_core::LockMode::Write => ("EXCLUSIVE", t.color(ColorRole::Error)),
                m31a_core::LockMode::Read => ("SHARED", t.color(ColorRole::Success)),
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{} {:2}. ", marker, idx + 1), row_style),
                Span::styled(
                    format!("[{}] ", mode_badge),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    entry.path.display().to_string(),
                    row_style.add_modifier(Modifier::BOLD),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::raw("      Holder: "),
                Span::styled(&entry.holder_id, t.style_info(false)),
                Span::raw("  Reason: "),
                Span::styled(&entry.reason, t.style_muted()),
                Span::raw(format!(
                    "  Acquired: {}",
                    entry.acquired_at.format("%H:%M:%S UTC")
                )),
            ]));
            lines.push(Line::raw(""));
        }
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_checkpoints_modal(
    f: &mut Frame,
    checkpoints: &[m31a_storage::CheckpointRecord],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 75, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Session Checkpoints  {} Total  ↑↓ Navigate Enter=Revert Esc=Close ",
        checkpoints.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)))
        .style(t.style_surface1());

    let mut lines = Vec::new();
    if checkpoints.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No checkpoints saved",
            t.style_muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  Create via `create_checkpoint` tool or milestones",
            t.style_muted(),
        )));
    } else {
        for (idx, cp) in checkpoints.iter().enumerate() {
            let is_sel = idx == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let marker = if is_sel { "▸ " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(format!("{} {:2}. ", marker, idx + 1), row_style),
                Span::styled(&cp.id, t.style_warning(true)),
                Span::raw(" — "),
                Span::styled(&cp.label, row_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" (Turn {})", cp.turn_index), t.style_muted()),
            ]));

            let git_info = cp.git_commit.as_deref().unwrap_or("none");
            let files_summary = if cp.modified_files.is_empty() {
                "none".to_string()
            } else {
                cp.modified_files.join(", ")
            };

            lines.push(Line::from(vec![
                Span::raw("      Git: "),
                Span::styled(git_info, t.style_success(false)),
                Span::raw("  Files: "),
                Span::styled(files_summary, t.style_muted()),
            ]));
            lines.push(Line::raw(""));
        }
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_revert_confirm_modal(
    f: &mut Frame,
    checkpoint: &m31a_storage::CheckpointRecord,
    selected: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(72, 45, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            " Confirm Revert ",
            t.style_warning(true),
        )]))
        .border_style(t.style_warning(false))
        .style(t.style_surface1());

    let options = ["Confirm Revert", "Cancel"];
    let opt_spans: Vec<Span> = options
        .iter()
        .enumerate()
        .map(|(i, &opt)| {
            if i == selected {
                Span::styled(format!("  {}  ", opt), t.style_selected())
            } else {
                Span::styled(format!("  {}  ", opt), t.style_primary())
            }
        })
        .collect();

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Checkpoint: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(&checkpoint.id, t.style_warning(true)),
            Span::raw(" (Turn "),
            Span::styled(checkpoint.turn_index.to_string(), t.style_primary()),
            Span::raw(")"),
        ]),
        Line::from(vec![
            Span::styled("Label: ", t.style_secondary().add_modifier(Modifier::BOLD)),
            Span::styled(&checkpoint.label, t.style_primary()),
        ]),
    ];

    if let Some(ref sha) = checkpoint.git_commit {
        lines.push(Line::from(vec![
            Span::styled(
                "Git SHA: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(sha, t.style_success(true)),
        ]));
    }

    if !checkpoint.modified_files.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "Modified Files: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(checkpoint.modified_files.join(", "), t.style_muted()),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "⚠ This will revert all subsequent conversation turns and working tree changes.",
        t.style_error(false),
    )));
    if checkpoint.git_commit.is_some() {
        lines.push(Line::from(Span::styled(
            "A safety backup ref will be created in refs/m31a/backups/.",
            t.style_muted(),
        )));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(opt_spans));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_analytics_dashboard(
    f: &mut Frame,
    metrics: &m31a_analytics::SessionTokenMetrics,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 80, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " Session Analytics ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Esc=Close ", t.style_muted()),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let lines = m31a_analytics::AnalyticsDashboard::build_lines(
        metrics,
        popup_area.width.saturating_sub(4) as usize,
    );
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_experimental_features(
    f: &mut Frame,
    items: &[(
        m31a_core::features::FeatureKey,
        &'static str,
        &'static str,
        bool,
    )],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(70, 75, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " Experimental Features ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ↑↓ Space=Toggle Enter=Toggle Esc=Close ", t.style_muted()),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, (key, name, desc, enabled))| {
            let is_sel = i == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let checkbox = if *enabled { "☑" } else { "☐" };
            let status = match key.default_stage() {
                m31a_core::features::FeatureStage::Experimental { .. } => "Experimental",
                m31a_core::features::FeatureStage::UnderDevelopment => "Under Development",
                _ => "Stable",
            };

            Line::from(vec![
                Span::styled(if is_sel { "▸ " } else { "  " }, row_style),
                Span::styled(
                    format!("{} ", checkbox),
                    if *enabled {
                        t.style_success(true)
                    } else {
                        t.style_muted()
                    },
                ),
                Span::styled(*name, row_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", status), t.style_muted()),
                Span::raw("\n    "),
                Span::styled(*desc, t.style_muted()),
            ])
        })
        .collect();

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_interactive_diff(
    f: &mut Frame,
    state: &m31a_diff::DiffViewerState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(88, 85, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Diff: {}  +{} −{}  ↑↓ PgUp/PgDn Esc=Close ",
        state.file_path, state.stats.lines_added, state.stats.lines_deleted
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Success)))
        .style(t.style_surface1());

    let visible_lines = popup_area.height.saturating_sub(2) as usize;
    let slice = if state.scroll_offset < state.lines.len() {
        &state.lines[state.scroll_offset..]
    } else {
        &[]
    };
    let lines = m31a_diff::DiffRenderer::render_lines(slice, visible_lines);
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_question_prompt(
    f: &mut Frame,
    q: &m31a_elicitation::QuestionItem,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(70, 50, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " Agent Question ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ↑↓ Space=Toggle Enter=Submit Esc=Cancel ", t.style_muted()),
        ]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)))
        .style(t.style_surface1());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Question: ", t.style_warning(true)),
            Span::styled(&q.question, t.style_primary()),
        ]),
        Line::raw(""),
    ];

    for (i, opt) in q.options.iter().enumerate() {
        let is_cursor = i == q.cursor_idx && !q.is_custom_focused;
        let is_selected = q.selected_indices.contains(&i);

        let marker = if q.is_multi_select {
            if is_selected { "☑" } else { "☐" }
        } else {
            if is_selected { "◉" } else { "○" }
        };

        let row_style = if is_cursor {
            t.style_selected()
        } else if is_selected {
            t.style_success(true)
        } else {
            t.style_primary()
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} ", marker), row_style),
            Span::styled(opt, row_style),
        ]));
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

fn render_processes_modal(
    f: &mut Frame,
    processes: &[m31a_proc::ProcessInfo],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(88, 75, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " Managed Processes ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ↑↓ Enter=View k=Kill Esc=Close ", t.style_muted()),
        ]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)))
        .style(t.style_surface1());

    if processes.is_empty() {
        let text = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "  No persistent background processes running.",
                t.style_muted(),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::raw("  Use "),
                Span::styled("start_background_process", t.style_accent(true)),
                Span::raw(" tool to launch servers."),
            ]),
            Line::raw(""),
            Line::from(Span::styled("  Press Esc to return.", t.style_muted())),
        ];
        let p = Paragraph::new(text).block(block);
        f.render_widget(p, popup_area);
        return;
    }

    let items: Vec<ListItem> = processes
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_sel = i == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let marker = if is_sel { "▸ " } else { "  " };

            let (badge_text, badge_color_str) = p.status.display_badge();
            let badge_color = match badge_color_str {
                "green" => t.color(ColorRole::Success),
                "cyan" => t.color(ColorRole::Info),
                "red" => t.color(ColorRole::Error),
                "yellow" => t.color(ColorRole::Warning),
                _ => t.color(ColorRole::TextSecondary),
            };

            let pid_str = p
                .pid
                .map(|n| format!("PID {:<6}", n))
                .unwrap_or_else(|| "PID —    ".into());
            let uptime_str = if p.uptime_seconds >= 60 {
                format!("{}m {}s", p.uptime_seconds / 60, p.uptime_seconds % 60)
            } else {
                format!("{}s", p.uptime_seconds)
            };

            let line1 = Line::from(vec![
                Span::styled(format!("{} {:2}. ", marker, i + 1), row_style),
                Span::styled(&p.label, row_style.add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(
                    badge_text,
                    Style::default()
                        .fg(badge_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(pid_str, t.style_muted()),
                Span::raw("  "),
                Span::styled(uptime_str, t.style_muted()),
            ]);

            let cwd_display = p.cwd.to_string_lossy().into_owned();
            let line2 = Line::from(vec![
                Span::raw("      "),
                Span::styled(&p.command, t.style_muted()),
                Span::raw("  "),
                Span::styled(cwd_display, t.style_muted()),
            ]);

            ListItem::new(vec![line1, line2])
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, popup_area);
}

fn render_process_viewer(
    f: &mut Frame,
    process_id: &str,
    label: &str,
    lines: &[m31a_proc::ProcessLine],
    scroll: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(88, 85, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Process: {} ({})  ↑↓ PgUp/PgDn Esc=Close ",
        label, process_id
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let visible_h = popup_area.height.saturating_sub(2) as usize;
    let slice = if scroll < lines.len() {
        &lines[scroll..]
    } else {
        &[]
    };
    let content_lines: Vec<Line> = slice
        .iter()
        .take(visible_h)
        .map(|pl| {
            let stream_badge = match pl.stream {
                m31a_proc::buffer::OutputStream::Stdout => "",
                m31a_proc::buffer::OutputStream::Stderr => "[ERR] ",
                m31a_proc::buffer::OutputStream::System => "[SYS] ",
            };
            Line::from(Span::styled(
                format!("{}{}", stream_badge, pl.line),
                t.style_primary(),
            ))
        })
        .collect();

    let p = Paragraph::new(content_lines).block(block);
    f.render_widget(p, popup_area);

    if lines.len() > visible_h {
        use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
        let mut scrollbar_state =
            ScrollbarState::new(lines.len().saturating_sub(visible_h)).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(t.style_muted());
        f.render_stateful_widget(scrollbar, popup_area, &mut scrollbar_state);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_command_palette(
    f: &mut Frame,
    query: &str,
    selected_index: usize,
    scroll_offset: usize,
    items: &[crate::modals::CommandPaletteItem],
    filtered_indices: &[usize],
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(78, 75, area);
    f.render_widget(Clear, popup_area);

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .border_style(t.border_focus())
        .title(Line::from(vec![
            Span::styled(
                format!(" {} ", crate::icons::Icon::Command.render(t)),
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Command Palette",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  [ESC to close]", t.style_muted()),
        ]))
        .style(t.style_surface1());

    f.render_widget(main_block, popup_area);

    let geometry = crate::modals::CommandPaletteGeometry::compute(popup_area);
    if geometry.list_inner_width() < 5 || geometry.list_inner_height() < 1 {
        return;
    }

    let search_area = geometry.search;
    let list_area = geometry.list;
    let details_area = geometry.details;
    let footer_area = geometry.footer;

    // 1. Search Bar
    let search_icon = crate::icons::Icon::Search.render(t);
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_subtle())
        .style(t.style_surface0());

    let search_text = if query.is_empty() {
        Line::from(vec![
            Span::styled(format!(" {} ", search_icon), t.style_muted()),
            Span::styled(
                "Type to search commands or press ↑↓ to navigate...",
                t.style_muted(),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!(" {} ", search_icon), t.style_accent(true)),
            Span::styled(query, t.style_primary().add_modifier(Modifier::BOLD)),
            Span::styled("█", t.style_accent(false)),
        ])
    };
    f.render_widget(Paragraph::new(search_text).block(search_block), search_area);

    // Render list
    let list_inner_h = geometry.list_inner_height();
    if filtered_indices.is_empty() {
        let no_res_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle());
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("  {} ", crate::icons::Icon::AlertTriangle.render(t)),
                    t.style_warning(false),
                ),
                Span::styled(
                    format!("No matching commands for \"{}\"", query),
                    t.style_muted(),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Try a different keyword or press Esc to return.",
                t.style_muted(),
            )),
        ])
        .block(no_res_block);
        f.render_widget(msg, list_area);
    } else {
        let start = scroll_offset.min(filtered_indices.len());
        let end = (start + list_inner_h).min(filtered_indices.len());
        let visible_slice = if start < end {
            &filtered_indices[start..end]
        } else {
            &[]
        };

        let list_items: Vec<ListItem> = visible_slice
            .iter()
            .enumerate()
            .filter_map(|(rel_idx, &item_idx)| {
                let item = items.get(item_idx)?;
                let actual_idx = scroll_offset + rel_idx;
                let is_sel = actual_idx == selected_index;
                let row_style = if is_sel {
                    t.style_selected()
                } else {
                    t.style_primary()
                };

                let sel_marker = if is_sel { "▸ " } else { "  " };
                let icon_glyph = item.icon.render(t);

                let mut spans = vec![
                    Span::styled(sel_marker, row_style),
                    Span::styled(
                        format!("{} ", icon_glyph),
                        if is_sel {
                            row_style
                        } else {
                            t.style_accent(false)
                        },
                    ),
                    Span::styled(
                        &item.title,
                        row_style.add_modifier(if is_sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                ];

                if let Some(ref sc) = item.shortcut {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("[{}]", sc),
                        if is_sel { row_style } else { t.style_muted() },
                    ));
                }

                Some(ListItem::new(Line::from(spans)))
            })
            .collect();

        let count_str = format!(" [{}/{}] ", selected_index + 1, filtered_indices.len());
        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(Line::from(vec![
                Span::styled(
                    " Commands ",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                ),
                Span::styled(count_str, t.style_muted()),
            ]));

        let list = List::new(list_items).block(list_block);
        f.render_widget(list, list_area);

        // Scrollbar if needed
        if filtered_indices.len() > list_inner_h && list_inner_h > 0 {
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let mut scrollbar_state =
                ScrollbarState::new(filtered_indices.len().saturating_sub(list_inner_h))
                    .position(scroll_offset);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█")
                .style(t.style_muted());
            f.render_stateful_widget(
                scrollbar,
                list_area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    // Render details pane
    if let Some(d_area) = details_area {
        let details_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_subtle())
            .title(Line::from(vec![Span::styled(
                " Command Details ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            )]))
            .style(t.style_surface1());

        let selected_item = filtered_indices
            .get(selected_index)
            .and_then(|&idx| items.get(idx));

        let details_content = if let Some(item) = selected_item {
            vec![
                Line::from(vec![
                    Span::styled(format!("{} ", item.icon.render(t)), t.style_accent(true)),
                    Span::styled(&item.title, t.style_primary().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(Span::styled(&item.description, t.style_secondary())),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Category: ", t.style_muted()),
                    Span::styled(&item.category, t.style_accent(false)),
                ]),
                Line::from(vec![
                    Span::styled("Shortcut: ", t.style_muted()),
                    Span::styled(
                        item.shortcut.as_deref().unwrap_or("None"),
                        t.style_primary(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Action ID: ", t.style_muted()),
                    Span::styled(&item.id, t.style_muted()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to execute this action",
                    t.style_success(true),
                )),
            ]
        } else {
            vec![Line::from(Span::styled(
                "No command selected.",
                t.style_muted(),
            ))]
        };

        f.render_widget(Paragraph::new(details_content).block(details_block), d_area);
    }

    // 3. Footer
    let footer_text = Line::from(vec![
        Span::styled("↑↓", t.style_accent(true)),
        Span::styled(" Navigate  ", t.style_muted()),
        Span::styled("Enter", t.style_accent(true)),
        Span::styled(" Execute  ", t.style_muted()),
        Span::styled("Esc", t.style_accent(true)),
        Span::styled(" Close  ", t.style_muted()),
        Span::styled("Type", t.style_accent(true)),
        Span::styled(" to filter", t.style_muted()),
    ]);
    f.render_widget(Paragraph::new(footer_text), footer_area);
}

fn render_setup_wizard(
    f: &mut Frame,
    state: &crate::modals::SetupWizardState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(78, 80, area);
    f.render_widget(Clear, popup_area);

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {} ", crate::icons::Icon::Wrench.render(t)),
            t.style_secondary(),
        ),
        Span::styled(
            "M31A Setup Wizard",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " [Step {}/8: {}]",
                (state.step as usize) + 1,
                state.step.title()
            ),
            t.style_accent(true),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .border_style(t.border_focus())
        .title(title_line)
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.width < 10 || inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(inner);

    let step_idx = state.step as usize;
    let step_spans: Vec<Span> = crate::modals::SetupStep::ALL
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let is_current = i == step_idx;
            let is_done = i < step_idx;
            let style = if is_current {
                t.style_selected().add_modifier(Modifier::BOLD)
            } else if is_done {
                t.style_success(false)
            } else {
                t.style_muted()
            };
            let symbol = if is_done {
                "✓"
            } else if is_current {
                "●"
            } else {
                "○"
            };
            vec![
                Span::styled(
                    format!(
                        " {} {} ",
                        symbol,
                        s.title().split_whitespace().next().unwrap_or("")
                    ),
                    style,
                ),
                if i + 1 < crate::modals::SetupStep::ALL.len() {
                    Span::styled("›", t.style_muted())
                } else {
                    Span::raw("")
                },
            ]
        })
        .collect();
    let step_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(t.border_subtle());
    f.render_widget(
        Paragraph::new(Line::from(step_spans)).block(step_block),
        chunks[0],
    );

    let content_area = chunks[1];
    match state.step {
        crate::modals::SetupStep::Welcome => {
            let mut lines = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "  Welcome to the M31A Autonomous Software Engineering Runtime!",
                    t.style_primary().add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(Span::styled(
                    "  This interactive setup wizard will guide you through configuring:",
                    t.style_secondary(),
                )),
                Line::from(Span::styled(
                    "    1. AI Model Provider (NVIDIA NIM, OpenAI, Anthropic, Gemini, Local Ollama, etc.)",
                    t.style_primary(),
                )),
                Line::from(Span::styled(
                    "    2. API Credentials & Authentication (securely masked)",
                    t.style_primary(),
                )),
                Line::from(Span::styled(
                    "    3. Default Model Selection from the canonical catalog",
                    t.style_primary(),
                )),
                Line::from(Span::styled(
                    "    4. Execution Security (Container Sandbox & Human Approval policies)",
                    t.style_primary(),
                )),
                Line::from(Span::styled(
                    "    5. Live Provider Connectivity Verification",
                    t.style_primary(),
                )),
                Line::from(""),
            ];
            if state.existing_config_detected {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(
                            "  {} Existing configuration detected.",
                            crate::icons::Icon::Info.render(t)
                        ),
                        t.style_warning(true),
                    ),
                    Span::styled(
                        " Running setup will update your configuration file safely.",
                        t.style_secondary(),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} First-time initialization.", crate::icons::Icon::Sparkles.render(t)), t.style_success(true)),
                    Span::styled(" A clean configuration file will be created in your standard config directory.", t.style_secondary()),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Press [Enter] to begin, or [Esc] to cancel.",
                t.style_accent(true),
            )));
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Provider => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "  Select the AI model provider to connect to M31A:",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for (idx, p) in state.providers.iter().enumerate() {
                let is_sel = idx == state.selected_provider_idx;
                let marker = if is_sel { "▸ " } else { "  " };
                let style = if is_sel {
                    t.style_selected().add_modifier(Modifier::BOLD)
                } else {
                    t.style_primary()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}{:<22}", marker, p.display_name()), style),
                    Span::styled(
                        format!("Default: {}", p.default_api_base()),
                        if is_sel { style } else { t.style_muted() },
                    ),
                ]));
            }
            lines.push(Line::from(""));
            let endpoint_style = if state.editing_endpoint {
                t.style_selected()
            } else {
                t.style_primary()
            };
            lines.push(Line::from(vec![
                Span::styled("  Endpoint URL: ", t.style_secondary()),
                Span::styled(&state.custom_endpoint, endpoint_style),
                if state.editing_endpoint {
                    Span::styled("█", t.style_accent(false))
                } else {
                    Span::raw("")
                },
                Span::styled("  [Tab: Edit endpoint]", t.style_muted()),
            ]));
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Credentials => {
            let is_local = state.current_provider().is_local();
            let mut lines = vec![
                Line::from(Span::styled(
                    format!(
                        "  Authentication credentials for {}:",
                        state.current_provider().display_name()
                    ),
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            if is_local {
                lines.push(Line::from(Span::styled(
                    "  Local providers (Ollama / LM Studio) typically do not require an API key.",
                    t.style_success(false),
                )));
                lines.push(Line::from(""));
            }
            if state.has_existing_key {
                let p_style = if state.preserve_existing_key {
                    t.style_selected().add_modifier(Modifier::BOLD)
                } else {
                    t.style_primary()
                };
                let n_style = if !state.preserve_existing_key {
                    t.style_selected().add_modifier(Modifier::BOLD)
                } else {
                    t.style_primary()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        if state.preserve_existing_key {
                            "  ▸ "
                        } else {
                            "    "
                        },
                        p_style,
                    ),
                    Span::styled("Preserve existing configured API Key (●●●●●●●●)", p_style),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        if !state.preserve_existing_key {
                            "  ▸ "
                        } else {
                            "    "
                        },
                        n_style,
                    ),
                    Span::styled("Enter new API Key", n_style),
                ]));
                lines.push(Line::from(""));
            }
            if !state.has_existing_key || !state.preserve_existing_key {
                let masked_input: String = "●".repeat(state.api_key_input.chars().count());
                lines.push(Line::from(vec![
                    Span::styled(
                        "  API Key: ",
                        t.style_secondary().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if masked_input.is_empty() {
                            "Paste or type API key here...".to_string()
                        } else {
                            masked_input
                        },
                        if state.api_key_input.is_empty() {
                            t.style_muted()
                        } else {
                            t.style_selected()
                        },
                    ),
                    Span::styled("█", t.style_accent(false)),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  [Key values are securely masked and never written to transcripts or logs]",
                    t.style_muted(),
                )));
            }
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Model => {
            let mut lines = vec![
                Line::from(Span::styled(
                    format!(
                        "  Select the default model for {}:",
                        state.current_provider().display_name()
                    ),
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for (idx, m) in state.models_for_provider.iter().enumerate() {
                let is_sel = idx == state.selected_model_idx && !state.editing_custom_model;
                let marker = if is_sel { "▸ " } else { "  " };
                let style = if is_sel {
                    t.style_selected().add_modifier(Modifier::BOLD)
                } else {
                    t.style_primary()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}{:<28}", marker, m.display_name), style),
                    Span::styled(
                        format!("({:<36}) [{}k]", m.slug, m.context_window / 1024),
                        if is_sel { style } else { t.style_muted() },
                    ),
                ]));
            }
            lines.push(Line::from(""));
            let custom_style = if state.editing_custom_model {
                t.style_selected().add_modifier(Modifier::BOLD)
            } else {
                t.style_primary()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if state.editing_custom_model {
                        "  ▸ "
                    } else {
                        "    "
                    },
                    custom_style,
                ),
                Span::styled("Custom Model Slug: ", t.style_secondary()),
                Span::styled(&state.custom_model_name, custom_style),
                if state.editing_custom_model {
                    Span::styled("█", t.style_accent(false))
                } else {
                    Span::raw("")
                },
                Span::styled("  [Tab: Toggle custom model]", t.style_muted()),
            ]));
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Security => {
            let sb_sel = state.security_field_idx == 0;
            let ap_sel = state.security_field_idx == 1;

            let lines = vec![
                Line::from(Span::styled(
                    "  Configure execution security boundaries & governance:",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        if sb_sel { "  ▸ " } else { "    " },
                        if sb_sel {
                            t.style_selected()
                        } else {
                            t.style_muted()
                        },
                    ),
                    Span::styled(
                        "Sandbox Policy:   ",
                        if sb_sel {
                            t.style_selected().add_modifier(Modifier::BOLD)
                        } else {
                            t.style_secondary()
                        },
                    ),
                    Span::styled(
                        format!("{:?}", state.sandbox_policy),
                        if sb_sel {
                            t.style_selected().add_modifier(Modifier::BOLD)
                        } else {
                            t.style_primary()
                        },
                    ),
                    Span::styled("  (Use ←/→ or Space to change)", t.style_muted()),
                ]),
                Line::from(Span::styled(
                    "      • Isolated (Recommended): Bubblewrap container restricts disk/network access",
                    t.style_muted(),
                )),
                Line::from(Span::styled(
                    "      • DangerouslyDisabled: Direct uncontained host execution",
                    t.style_muted(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        if ap_sel { "  ▸ " } else { "    " },
                        if ap_sel {
                            t.style_selected()
                        } else {
                            t.style_muted()
                        },
                    ),
                    Span::styled(
                        "Approval Policy:  ",
                        if ap_sel {
                            t.style_selected().add_modifier(Modifier::BOLD)
                        } else {
                            t.style_secondary()
                        },
                    ),
                    Span::styled(
                        format!("{:?}", state.approval_policy),
                        if ap_sel {
                            t.style_selected().add_modifier(Modifier::BOLD)
                        } else {
                            t.style_primary()
                        },
                    ),
                    Span::styled("  (Use ←/→ or Space to change)", t.style_muted()),
                ]),
                Line::from(Span::styled(
                    "      • OnRequest (Recommended): Prompt before executing mutating shell commands or file edits",
                    t.style_muted(),
                )),
                Line::from(Span::styled(
                    "      • ApproveForMe: Autonomous execution mode; tools run without blocking for confirmation",
                    t.style_muted(),
                )),
                Line::from(Span::styled(
                    "      • Never: Strict read-only mode; reject all mutation tool executions",
                    t.style_muted(),
                )),
            ];
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Connectivity => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "  Provider Connectivity Verification:",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Endpoint: ", t.style_muted()),
                    Span::styled(state.effective_endpoint(), t.style_primary()),
                ]),
                Line::from(vec![
                    Span::styled("  Model:    ", t.style_muted()),
                    Span::styled(state.effective_model(), t.style_primary()),
                ]),
                Line::from(""),
            ];

            match state.connectivity_status {
                crate::modals::ConnectivityStatus::NotTested => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Status: Ready to test.",
                                crate::icons::Icon::Info.render(t)
                            ),
                            t.style_muted(),
                        ),
                        Span::styled(
                            " Press [Enter] or [t] to probe live connection.",
                            t.style_primary(),
                        ),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  (Or press [s] to skip connectivity test and proceed to review)",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::Validating => {
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "  {} Validating provider credentials and configuration...",
                            crate::icons::Icon::LoaderCircle.render(t)
                        ),
                        t.style_accent(true),
                    )]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Checking local schemas and endpoint parameters...",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::TestingConnection => {
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "  {} Connecting to provider endpoint...",
                            crate::icons::Icon::LoaderCircle.render(t)
                        ),
                        t.style_accent(true),
                    )]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Verifying network reachability and endpoint authentication...",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::CheckingModel => {
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "  {} Verifying model availability for `{}`...",
                            crate::icons::Icon::LoaderCircle.render(t),
                            state.effective_model()
                        ),
                        t.style_accent(true),
                    )]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Confirming model catalog registration and inference capability...",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::Connected => {
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "  {} Provider endpoint verified successfully!",
                            crate::icons::Icon::CircleCheck.render(t)
                        ),
                        t.style_success(true),
                    )]));
                    lines.push(Line::from(""));

                    if let Some(ref details) = state.connectivity_details {
                        let lat_str = details
                            .latency_ms
                            .map(|ms| format!(" ({ms} ms)"))
                            .unwrap_or_default();
                        lines.push(Line::from(vec![
                            Span::styled("  • Provider connectivity: ", t.style_muted()),
                            Span::styled(format!("Verified{}", lat_str), t.style_success(false)),
                        ]));

                        if details.model_verified {
                            lines.push(Line::from(vec![
                                Span::styled("  • Model availability:    ", t.style_muted()),
                                Span::styled("Confirmed", t.style_success(false)),
                            ]));
                        } else if details.model_check_unsupported {
                            lines.push(Line::from(vec![
                                Span::styled("  • Model availability:    ", t.style_muted()),
                                Span::styled(
                                    "Not independently verified (Provider does not expose model catalog probe)",
                                    t.style_muted(),
                                ),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled("  • Model availability:    ", t.style_muted()),
                                Span::styled(
                                    "Unconfirmed (Check timed out or was inconclusive)",
                                    t.style_warning(false),
                                ),
                            ]));
                        }
                    } else {
                        lines.push(Line::from(Span::styled(
                            "  Provider connectivity established.",
                            t.style_secondary(),
                        )));
                    }

                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Press [Enter] to proceed to configuration review.",
                        t.style_accent(true),
                    )));
                }
                crate::modals::ConnectivityStatus::AuthenticationFailed => {
                    let err = state
                        .connectivity_error
                        .as_deref()
                        .unwrap_or("Authentication failed (HTTP 401/403). Check API key.");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Authentication Failed: ",
                                crate::icons::Icon::CircleX.render(t)
                            ),
                            t.style_error(true),
                        ),
                        Span::styled(err, t.style_error(false)),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  The provider rejected your credentials. Press [Esc] to return to Credentials step.",
                        t.style_warning(false),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Press [r] to retry, [Esc] to adjust settings, or [s] to skip verification.",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::ModelUnsupported => {
                    let err = state
                        .connectivity_error
                        .as_deref()
                        .unwrap_or("Model unsupported or unavailable at this endpoint.");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Model Unsupported: ",
                                crate::icons::Icon::CircleX.render(t)
                            ),
                            t.style_error(true),
                        ),
                        Span::styled(err, t.style_error(false)),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  The endpoint is reachable, but does not provide or support this model ID.",
                        t.style_warning(false),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Press [r] to retry, [Esc] to select another model, or [s] to skip verification.",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::TimedOut => {
                    let err = state
                        .connectivity_error
                        .as_deref()
                        .unwrap_or("Probe timed out after 8s.");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Connection Timed Out: ",
                                crate::icons::Icon::CircleX.render(t)
                            ),
                            t.style_error(true),
                        ),
                        Span::styled(err, t.style_error(false)),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Provider did not respond in time. Check endpoint responsiveness or firewall.",
                        t.style_warning(false),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Press [r] to retry, [Esc] to adjust endpoint, or [s] to skip verification.",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::Unreachable => {
                    let err = state
                        .connectivity_error
                        .as_deref()
                        .unwrap_or("Host unreachable. Network failure or DNS resolution error.");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Host Unreachable: ",
                                crate::icons::Icon::CircleX.render(t)
                            ),
                            t.style_error(true),
                        ),
                        Span::styled(err, t.style_error(false)),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Could not establish TCP connection to endpoint. Check network connection and URL.",
                        t.style_warning(false),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Press [r] to retry, [Esc] to adjust endpoint, or [s] to skip verification.",
                        t.style_muted(),
                    )));
                }
                crate::modals::ConnectivityStatus::Failed => {
                    let err = state
                        .connectivity_error
                        .as_deref()
                        .unwrap_or("Network or configuration error");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!(
                                "  {} Verification failed: ",
                                crate::icons::Icon::CircleX.render(t)
                            ),
                            t.style_error(true),
                        ),
                        Span::styled(err, t.style_error(false)),
                    ]));
                    lines.push(Line::from(""));
                    if let Some(ref details) = state.connectivity_details
                        && details.endpoint_verified
                    {
                        lines.push(Line::from(vec![
                            Span::styled("  • Endpoint Reachability: ", t.style_muted()),
                            Span::styled("Verified", t.style_success(false)),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled("  • Model Availability:    ", t.style_muted()),
                            Span::styled("Failed", t.style_error(false)),
                        ]));
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(Span::styled(
                        "  Press [r] to retry, [Esc] to adjust settings, or [s] to skip and continue anyway.",
                        t.style_muted(),
                    )));
                    lines.push(Line::from(Span::styled(
                        "  (Note: Enter is disabled on failure to prevent accidental unverified progression)",
                        t.style_muted(),
                    )));
                }
            }
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Review => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "  Review Configuration Summary before saving:",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  • Provider:        ", t.style_muted()),
                    Span::styled(
                        state.current_provider().display_name(),
                        t.style_primary().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • API Base URL:    ", t.style_muted()),
                    Span::styled(state.effective_endpoint(), t.style_primary()),
                ]),
                Line::from(vec![
                    Span::styled("  • API Key:         ", t.style_muted()),
                    Span::styled(
                        if state.preserve_existing_key {
                            "[Existing Key Preserved]"
                        } else if !state.api_key_input.is_empty() {
                            "[New Key Configured (Masked)]"
                        } else if state.current_provider().is_local() {
                            "[None Required - Local]"
                        } else {
                            "[Missing]"
                        },
                        t.style_success(false),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • Default Model:   ", t.style_muted()),
                    Span::styled(
                        state.effective_model(),
                        t.style_primary().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  • Sandbox Policy:  ", t.style_muted()),
                    Span::styled(format!("{:?}", state.sandbox_policy), t.style_primary()),
                ]),
                Line::from(vec![
                    Span::styled("  • Approval Policy: ", t.style_muted()),
                    Span::styled(format!("{:?}", state.approval_policy), t.style_primary()),
                ]),
                Line::from(vec![
                    Span::styled("  • Connectivity:    ", t.style_muted()),
                    if state.skipped_verification {
                        Span::styled("[Verification Skipped by User]", t.style_warning(true))
                    } else if state.connectivity_status.is_success() {
                        if let Some(ref details) = state.connectivity_details {
                            if details.model_verified {
                                Span::styled(
                                    "[Verified: Provider & Model OK]",
                                    t.style_success(true),
                                )
                            } else if details.model_check_unsupported {
                                Span::styled(
                                    "[Verified: Endpoint OK (Model Not Independently Verified)]",
                                    t.style_success(false),
                                )
                            } else {
                                Span::styled("[Verified: Endpoint OK]", t.style_success(false))
                            }
                        } else {
                            Span::styled("[Verified]", t.style_success(true))
                        }
                    } else if state.connectivity_status.is_failure() {
                        Span::styled(format!("[{}]", state.connectivity_status.label()), t.style_error(true))
                    } else {
                        Span::styled("[Not Tested]", t.style_warning(false))
                    },
                ]),
                Line::from(""),
            ];

            if let Some(ref err) = state.save_error {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} Save Failed: ", crate::icons::Icon::CircleX.render(t)),
                        t.style_error(true),
                    ),
                    Span::styled(err, t.style_error(false)),
                ]));
                lines.push(Line::from(""));
            }

            lines.push(Line::from(Span::styled(
                "  Press [Enter] to write configuration to disk, or [Esc] to go back.",
                t.style_accent(true),
            )));
            f.render_widget(Paragraph::new(lines), content_area);
        }
        crate::modals::SetupStep::Complete => {
            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        format!("  {} ", crate::icons::Icon::CircleCheck.render(t)),
                        t.style_success(true),
                    ),
                    Span::styled(
                        "M31A Setup Completed Successfully!",
                        t.style_success(true).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Your configuration has been safely written and applied to the current session.",
                    t.style_primary(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Active Model: ", t.style_muted()),
                    Span::styled(
                        state.effective_model(),
                        t.style_primary().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Provider:     ", t.style_muted()),
                    Span::styled(state.current_provider().display_name(), t.style_primary()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press [Enter] or [Esc] to close the wizard and begin development.",
                    t.style_accent(true),
                )),
            ];
            f.render_widget(Paragraph::new(lines), content_area);
        }
    }

    let footer_text = match state.step {
        crate::modals::SetupStep::Welcome => "Enter: Begin  │  Esc: Cancel",
        crate::modals::SetupStep::Provider => {
            "↑↓: Select Provider  │  Tab: Custom Endpoint  │  Enter: Next  │  Esc: Back"
        }
        crate::modals::SetupStep::Credentials => {
            "Type: Enter API Key  │  Backspace: Delete  │  Enter: Next  │  Esc: Back"
        }
        crate::modals::SetupStep::Model => {
            "↑↓: Select Model  │  Tab: Custom Model  │  Enter: Next  │  Esc: Back"
        }
        crate::modals::SetupStep::Security => {
            "↑↓: Switch Setting  │  ←→/Space: Change Option  │  Enter: Next  │  Esc: Back"
        }
        crate::modals::SetupStep::Connectivity => {
            if state.connectivity_status.is_success() {
                "Enter: Proceed to Review  │  r: Retest  │  Esc: Back"
            } else if state.connectivity_status.is_failure() {
                "r: Retry  │  s: Skip Verification  │  Esc: Back (Enter disabled on failure)"
            } else {
                "Enter/t: Probe Connection  │  r: Retry  │  s: Skip Verification  │  Esc: Back"
            }
        }
        crate::modals::SetupStep::Review => "Enter: Confirm & Save  │  Esc: Back",
        crate::modals::SetupStep::Complete => "Enter/Esc: Return to Chat",
    };
    f.render_widget(
        Paragraph::new(Span::styled(format!("  {}", footer_text), t.style_muted())),
        chunks[2],
    );
}

fn render_reasoning_effort(
    f: &mut Frame,
    state: &crate::modals::ReasoningEffortState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(60, 48, area);
    f.render_widget(Clear, popup_area);

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {} ", crate::icons::Icon::Sliders.render(t)),
            t.style_secondary(),
        ),
        Span::styled(
            "Reasoning Effort",
            t.style_secondary().add_modifier(Modifier::BOLD),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .border_style(t.border_focus())
        .title(title_line)
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.width < 10 || inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let intro = Paragraph::new(Span::styled(
        "Choose how much reasoning / thinking effort the model should expend:",
        t.style_secondary(),
    ));
    f.render_widget(intro, chunks[0]);

    let items: Vec<ListItem> = state
        .options
        .iter()
        .enumerate()
        .map(|(idx, opt)| {
            let is_sel = idx == state.selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let style = if is_sel {
                t.style_selected().add_modifier(Modifier::BOLD)
            } else {
                t.style_primary()
            };

            let line = Line::from(vec![
                Span::styled(marker, style),
                Span::styled(
                    format!("{} ", opt.icon.render(t)),
                    if is_sel { style } else { t.style_accent(false) },
                ),
                Span::styled(format!("{:<10}", opt.label), style),
                Span::styled(
                    opt.description,
                    if is_sel { style } else { t.style_muted() },
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(t.style_selected());
    f.render_widget(list, chunks[1]);

    let current_str = state
        .current_setting
        .as_deref()
        .unwrap_or("None (provider default)");
    let current_line = Line::from(vec![
        Span::styled("Current Setting: ", t.style_muted()),
        Span::styled(current_str, t.style_primary().add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(current_line), chunks[2]);

    let footer = Line::from(vec![
        Span::styled("↑↓", t.style_accent(true)),
        Span::styled(" Navigate  ", t.style_muted()),
        Span::styled("Enter", t.style_accent(true)),
        Span::styled(" Apply  ", t.style_muted()),
        Span::styled("Esc", t.style_accent(true)),
        Span::styled(" Cancel", t.style_muted()),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[3]);
}

fn render_select_picker_modal(
    f: &mut Frame,
    picker: &crate::modals::SelectPickerState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(76, 75, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                format!(" {} ", picker.title),
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {} ", picker.query), t.style_accent(true)),
            Span::styled(
                " [↑↓] Select  │  [Enter] Choose  │  [Esc] Back ",
                t.style_muted(),
            ),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(inner);

    let left_items: Vec<ListItem> = picker
        .filtered_indices
        .iter()
        .enumerate()
        .filter_map(|(disp_idx, &real_idx)| {
            let item = picker.items.get(real_idx)?;
            let is_sel = disp_idx == picker.selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            let pointer = if is_sel { "▸ " } else { "  " };
            let mut spans = vec![
                Span::styled(pointer, row_style),
                Span::styled(&item.title, row_style.add_modifier(Modifier::BOLD)),
            ];

            if let Some(badge) = &item.badge {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(format!("[{badge}]"), t.style_secondary()));
            }

            if let Some(shortcut) = &item.shortcut {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(format!("({shortcut})"), t.style_muted()));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(&item.category, t.style_muted()));

            Some(ListItem::new(Line::from(spans)))
        })
        .collect();

    let list_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(t.border_subtle())
        .title(Span::styled(" Items ", t.style_muted()));
    let list_widget = List::new(left_items).block(list_block);
    f.render_widget(list_widget, chunks[0]);

    let detail_block = Block::default()
        .title(Span::styled(" Details ", t.style_muted()))
        .style(t.style_surface0());

    let mut detail_lines = Vec::new();
    if let Some(item) = picker.selected_item() {
        detail_lines.push(Line::from(vec![
            Span::styled("Title: ", t.style_secondary().add_modifier(Modifier::BOLD)),
            Span::styled(&item.title, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled(
                "Category: ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(&item.category, t.style_accent(false)),
        ]));
        if let Some(sc) = &item.shortcut {
            detail_lines.push(Line::from(vec![
                Span::styled(
                    "Shortcut: ",
                    t.style_secondary().add_modifier(Modifier::BOLD),
                ),
                Span::styled(sc, t.style_accent(true)),
            ]));
        }
        detail_lines.push(Line::raw(""));
        detail_lines.push(Line::from(vec![Span::styled(
            "Description:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]));
        for l in item.description.lines() {
            detail_lines.push(Line::from(vec![Span::styled(
                format!("  {l}"),
                t.style_primary(),
            )]));
        }
        if let Some(detail) = &item.detail {
            detail_lines.push(Line::raw(""));
            detail_lines.push(Line::from(vec![Span::styled(
                "Configuration / Details:",
                t.style_secondary().add_modifier(Modifier::BOLD),
            )]));
            for l in detail.lines() {
                detail_lines.push(Line::from(vec![Span::styled(
                    format!("  {l}"),
                    t.style_muted(),
                )]));
            }
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            "No items matched filter.",
            t.style_muted(),
        )));
    }

    let p = Paragraph::new(detail_lines)
        .block(detail_block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, chunks[1]);
}

fn render_settings_modal(
    f: &mut Frame,
    app: &App,
    active_tab: usize,
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(76, 75, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                " M31A Settings ",
                t.style_secondary().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " [Tab] Section  │  [↑↓] Select  │  [Space/Enter] Toggle  │  [Esc] Close ",
                t.style_muted(),
            ),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.height < 6 || inner.width < 20 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Tabs
            Constraint::Min(4),    // Setting items list
            Constraint::Length(1), // Footer status
        ])
        .split(inner);

    // 1. Render Tabs
    let tab_titles = [
        "1. General",
        "2. Security",
        "3. TUI & Theme",
        "4. Features & Agent",
    ];
    let mut tab_spans: Vec<Span> = Vec::new();
    for (idx, title) in tab_titles.iter().enumerate() {
        let is_active = idx == active_tab;
        let style = if is_active {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            t.style_muted()
        };
        tab_spans.push(Span::styled(format!("  {}  ", title), style));
        tab_spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

    // 2. Build setting items for active_tab
    let items_data: Vec<(&'static str, String, &'static str)> = match active_tab {
        0 => vec![
            (
                "Active Model",
                app.active_model.clone(),
                "Primary LLM engine for inference and tool orchestration",
            ),
            (
                "Reasoning Effort",
                app.config
                    .reasoning_effort
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                "Thinking depth override for reasoning models (none, low, medium, high)",
            ),
            (
                "Dynamic Model Routing",
                if app.config.routing_enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Route turns to specialized tier models based on prompt complexity",
            ),
            (
                "Web Search Engine",
                app.config.web_search.clone(),
                "Multi-engine technical doc and web grounding provider (live, duckduckgo, disabled)",
            ),
        ],
        1 => vec![
            (
                "Sandbox Isolation",
                format!("{:?}", app.config.sandbox),
                "Filesystem write restrictions (ReadOnly, WorkspaceWrite, DangerFullAccess)",
            ),
            (
                "Mutation Approval Policy",
                format!("{:?}", app.config.approval),
                "Gatekeeping for file edits and shell execution (OnRequest, ApproveForMe, Never)",
            ),
            (
                "Allow Login Shell",
                if app.config.allow_login_shell {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Permit user interactive login shell commands",
            ),
            (
                "Workspace Lifecycle Hooks",
                if app.config.hooks.enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Execute configured pre-turn, post-turn, and audit hooks",
            ),
        ],
        2 => vec![
            (
                "Color Theme",
                app.config
                    .tui
                    .theme
                    .clone()
                    .unwrap_or_else(|| "dark".to_string()),
                "Terminal color scheme (dark, light, contrast)",
            ),
            (
                "UI Animations & Spinners",
                if app.config.tui.animations {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Smooth spinner frames and streaming transitions",
            ),
            (
                "Vim Navigation Mode",
                if app.config.tui.vim_mode_default {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Default to modal Vim navigation in chat composer",
            ),
            (
                "Terminal Mouse Support",
                if app.config.tui.mouse_enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Enable or disable terminal mouse capture (disable for native terminal selection)",
            ),
            (
                "Scroll Acceleration",
                if app.config.tui.scroll_acceleration {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Accelerate scrolling speed during rapid mouse wheel turns",
            ),
            (
                "Auto-Copy on Select",
                if app.config.tui.copy_on_select {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Automatically copy highlighted text to system clipboard",
            ),
            (
                "Mouse Scroll Speed",
                format!("{:.1}x", app.config.tui.mouse_scroll_speed),
                "Multiplier factor for terminal mouse wheel scrolling",
            ),
        ],
        3 => vec![
            (
                "Long-Term Memories",
                if app.config.memories.enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Consolidate architectural learnings across sessions",
            ),
            (
                "Goal Tracking & Steering",
                if app.config.goals.enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Track high-level project goals and token spend budgets",
            ),
            (
                "Subagent Max Recursion Depth",
                format!("{}", app.config.agents.max_depth),
                "Maximum nested agent spawning depth limit (1, 2, 3, 5)",
            ),
            (
                "Subagent Max Concurrent Threads",
                format!("{}", app.config.agents.max_threads),
                "Maximum parallel agent workers allowed simultaneously (4, 8, 16, 32)",
            ),
            (
                "Code Mode Orchestration",
                if app
                    .config
                    .features
                    .get("code_mode")
                    .copied()
                    .unwrap_or(true)
                {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Programmatic TypeScript/Starlark tool scripts execution",
            ),
            (
                "Guardian Safety Supervisor",
                if app
                    .config
                    .features
                    .get("guardian_v2")
                    .copied()
                    .unwrap_or(true)
                {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                "Circuit-breaker sliding window autonomous supervisor",
            ),
        ],
        _ => Vec::new(),
    };

    let list_items: Vec<ListItem> = items_data
        .into_iter()
        .enumerate()
        .map(|(i, (name, val, desc))| {
            let is_sel = i == selected_index;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let desc_style = if is_sel {
                t.style_selected()
            } else {
                t.style_muted()
            };

            let val_style = match val.as_str() {
                "Enabled" | "WorkspaceWrite" => t.style_success(true),
                "DangerFullAccess" => t.style_warning(true),
                "Disabled" | "ReadOnly" | "Never" => t.style_muted(),
                _ => t.style_info(true),
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(if is_sel { "▸ " } else { "  " }, row_style),
                    Span::styled(name, row_style.add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(format!("[ {} ]", val), val_style),
                ]),
                Line::from(vec![Span::raw("    "), Span::styled(desc, desc_style)]),
            ])
        })
        .collect();

    let total_items = list_items.len();
    let list = List::new(list_items);
    f.render_widget(list, chunks[1]);

    let visible_h = chunks[1].height as usize / 2;
    if total_items > visible_h {
        use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
        let mut scrollbar_state = ScrollbarState::new(total_items.saturating_sub(visible_h))
            .position(selected_index.min(total_items.saturating_sub(1)));
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(t.style_muted());
        f.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    // 3. Render Footer
    let footer_line = Line::from(vec![
        Span::styled(" Config file: ", t.style_muted()),
        Span::styled(
            m31a_core::Config::config_path().display().to_string(),
            t.style_secondary(),
        ),
        Span::styled("  │  Changes auto-persisted to disk", t.style_muted()),
    ]);
    f.render_widget(Paragraph::new(footer_line), chunks[2]);
}

fn render_worktree_selector_modal(
    f: &mut Frame,
    worktrees: &[m31a_core::worktree::ManagedWorktree],
    selected_index: usize,
    current_cwd: &std::path::Path,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 75, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Managed Git Worktrees  {} Total  ↑↓ Navigate  Enter=Switch  n=New  Esc=Close ",
        worktrees.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(Style::default().fg(t.color(ColorRole::Info)))
        .style(t.style_surface1());

    let mut lines = Vec::new();
    if worktrees.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No managed worktrees found for this repository.",
            t.style_muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  Press 'n' to create an isolated worktree branch.",
            t.style_muted(),
        )));
    } else {
        for (idx, wt) in worktrees.iter().enumerate() {
            let is_sel = idx == selected_index;
            let is_active = wt.path == current_cwd;
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };
            let marker = if is_sel { "▸ " } else { "  " };
            let active_tag = if is_active { " [ACTIVE]" } else { "" };
            let branch_str = wt.branch.as_deref().unwrap_or("detached");
            let sha_short = if wt.head_sha.len() >= 8 {
                &wt.head_sha[..8]
            } else {
                &wt.head_sha
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{} {:2}. ", marker, idx + 1), row_style),
                Span::styled(
                    format!("{:<10} ", wt.id),
                    row_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("branch: {:<18} ", branch_str), t.style_secondary()),
                Span::styled(format!("head: {} ", sha_short), t.style_muted()),
                Span::styled(
                    active_tag,
                    Style::default()
                        .fg(t.color(ColorRole::Success))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("       "),
                Span::styled(format!("path: {}", wt.path.display()), t.style_muted()),
            ]));
            lines.push(Line::raw(""));
        }
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, popup_area);
}

// ===== New In-Depth Modal Renderers =====

fn render_mcp_hub_modal(
    f: &mut Frame,
    servers: &[crate::modals::McpServerDisplay],
    selected_index: usize,
    view_tools: bool,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 78, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Model Context Protocol (MCP) Hub  {} Servers  ↑↓ Navigate  Space=Toggle  Tab=Tools  Esc=Close ",
        servers.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.width < 30 || inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(inner);

    // Left pane: Server list
    let mut items = Vec::new();
    if servers.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  No MCP servers configured.",
            t.style_muted(),
        ))));
        items.push(ListItem::new(Line::from(Span::styled(
            "  Configure in ~/.m31a/config.toml under [mcp_servers].",
            t.style_muted(),
        ))));
    } else {
        for (idx, s) in servers.iter().enumerate() {
            let is_sel = idx == selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let (status_icon, status_color) = if s.enabled {
                ("● ACTIVE", t.color(ColorRole::Success))
            } else {
                ("○ DISABLED", t.color(ColorRole::TextMuted))
            };
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            items.push(ListItem::new(vec![
                Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(&s.name, row_style.add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" [{}]", s.transport), t.style_secondary()),
                    Span::raw(" "),
                    Span::styled(
                        status_icon,
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("tools: {} registered", s.tools_count),
                        t.style_muted(),
                    ),
                ]),
            ]));
        }
    }

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(t.border_default())
        .title(" Registered Servers ");
    let server_list = List::new(items).block(left_block);
    f.render_widget(server_list, chunks[0]);

    // Right pane: Selected server details
    let mut detail_lines = Vec::new();
    if let Some(s) = servers.get(selected_index) {
        detail_lines.push(Line::from(vec![
            Span::styled("Server Name: ", t.style_muted()),
            Span::styled(&s.name, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Transport:   ", t.style_muted()),
            Span::styled(&s.transport, t.style_secondary()),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Command:     ", t.style_muted()),
            Span::styled(&s.command, t.style_primary()),
        ]));
        detail_lines.push(Line::raw(""));

        detail_lines.push(Line::from(Span::styled(
            "Exposed Tools & Capabilities:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        if s.tools.is_empty() {
            detail_lines.push(Line::from(Span::styled(
                "  (No tools registered or server offline)",
                t.style_muted(),
            )));
        } else {
            for tool in &s.tools {
                detail_lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(t.color(ColorRole::Accent))),
                    Span::styled(tool, t.style_primary()),
                ]));
            }
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            "Select an MCP server from the left to inspect details.",
            t.style_muted(),
        )));
    }

    let right_block = Block::default()
        .borders(Borders::NONE)
        .title(if view_tools {
            " Tools Inspection "
        } else {
            " Server Overview "
        });
    let detail_p = Paragraph::new(detail_lines).block(right_block);
    f.render_widget(detail_p, chunks[1]);
}

fn render_skills_modal(
    f: &mut Frame,
    skills: &[m31a_core::skills::SkillInfo],
    selected_index: usize,
    search_query: &str,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 82, area);
    f.render_widget(Clear, popup_area);

    let title = if search_query.is_empty() {
        format!(
            " Skills Library  {} Discovered  ↑↓ Navigate  Enter=Inject  Esc=Close ",
            skills.len()
        )
    } else {
        format!(
            " Skills Library  Search: \"{}\" ({} Discovered)  ↑↓ Navigate  Enter=Inject  Esc=Close ",
            search_query,
            skills.len()
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.width < 30 || inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(inner);

    // Left pane: Skill list
    let mut items = Vec::new();
    if skills.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  No skills discovered.",
            t.style_muted(),
        ))));
        items.push(ListItem::new(Line::from(Span::styled(
            "  Place skill folders with SKILL.md in ~/.m31a/skills/ or .m31a/skills/.",
            t.style_muted(),
        ))));
    } else {
        for (idx, s) in skills.iter().enumerate() {
            let is_sel = idx == selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            items.push(ListItem::new(vec![
                Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(&s.name, row_style.add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        if s.description.chars().count() > 30 {
                            let truncated: String = s.description.chars().take(28).collect();
                            format!("{}…", truncated)
                        } else {
                            s.description.clone()
                        },
                        t.style_muted(),
                    ),
                ]),
            ]));
        }
    }

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(t.border_default())
        .title(" Discovered Skills ");
    let skill_list = List::new(items).block(left_block);
    f.render_widget(skill_list, chunks[0]);

    // Right pane: Selected skill detailed instructions
    let mut detail_lines = Vec::new();
    if let Some(s) = skills.get(selected_index) {
        detail_lines.push(Line::from(vec![
            Span::styled("Skill: ", t.style_muted()),
            Span::styled(&s.name, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Path:  ", t.style_muted()),
            Span::styled(s.path.display().to_string(), t.style_muted()),
        ]));
        detail_lines.push(Line::raw(""));
        detail_lines.push(Line::from(Span::styled(
            "Description:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        detail_lines.push(Line::from(Span::styled(
            format!("  {}", s.description),
            t.style_primary(),
        )));
        detail_lines.push(Line::raw(""));
        detail_lines.push(Line::from(Span::styled(
            "Skill Instructions & Directives:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));

        for l in s.instructions.lines().take(40) {
            detail_lines.push(Line::from(vec![
                Span::styled("  │ ", Style::default().fg(t.color(ColorRole::Border))),
                Span::styled(l.to_string(), t.style_primary()),
            ]));
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            "Select a skill from the list to preview its instructions.",
            t.style_muted(),
        )));
    }

    let right_block = Block::default()
        .borders(Borders::NONE)
        .title(" Skill Instructions & Manifest ");
    let detail_p = Paragraph::new(detail_lines).block(right_block);
    f.render_widget(detail_p, chunks[1]);
}

fn render_templates_modal(
    f: &mut Frame,
    templates: &[m31a_collaboration_templates::CollaborationTemplate],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 78, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Collaboration Session Templates  {} Available  ↑↓ Navigate  Enter=Launch  Esc=Close ",
        templates.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.width < 30 || inner.height < 6 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(inner);

    // Left pane: Templates list
    let mut items = Vec::new();
    for (idx, tmpl) in templates.iter().enumerate() {
        let is_sel = idx == selected_index;
        let marker = if is_sel { "▸ " } else { "  " };
        let row_style = if is_sel {
            t.style_selected()
        } else {
            t.style_primary()
        };
        let dur = tmpl
            .estimated_duration_minutes
            .map(|m| format!(" (~{}m)", m))
            .unwrap_or_default();

        items.push(ListItem::new(vec![
            Line::from(vec![
                Span::styled(marker, row_style),
                Span::styled(format!("{} ", tmpl.icon), Style::default()),
                Span::styled(&tmpl.name, row_style.add_modifier(Modifier::BOLD)),
                Span::styled(dur, t.style_muted()),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    if tmpl.description.chars().count() > 32 {
                        let truncated: String = tmpl.description.chars().take(30).collect();
                        format!("{}…", truncated)
                    } else {
                        tmpl.description.clone()
                    },
                    t.style_muted(),
                ),
            ]),
        ]));
    }

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(t.border_default())
        .title(" Preset Templates ");
    let tmpl_list = List::new(items).block(left_block);
    f.render_widget(tmpl_list, chunks[0]);

    // Right pane: Template specification & workflow steps
    let mut detail_lines = Vec::new();
    if let Some(tmpl) = templates.get(selected_index) {
        detail_lines.push(Line::from(vec![
            Span::styled("Template: ", t.style_muted()),
            Span::styled(
                format!("{} {}", tmpl.icon, tmpl.name),
                t.style_primary().add_modifier(Modifier::BOLD),
            ),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Category: ", t.style_muted()),
            Span::styled(format!("{:?}", tmpl.category), t.style_secondary()),
        ]));
        detail_lines.push(Line::raw(""));
        detail_lines.push(Line::from(Span::styled(
            "Summary:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        detail_lines.push(Line::from(Span::styled(
            format!("  {}", tmpl.description),
            t.style_primary(),
        )));
        detail_lines.push(Line::raw(""));

        detail_lines.push(Line::from(Span::styled(
            "Configured Rules & Presets:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        let mode_str = tmpl
            .config
            .collaboration_mode
            .map(|m| format!("{:?}", m))
            .unwrap_or_else(|| "Default".to_string());
        let sandbox_str = tmpl
            .config
            .sandbox_policy
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "WorkspaceWrite".to_string());
        detail_lines.push(Line::from(vec![
            Span::styled("  • Mode: ", t.style_muted()),
            Span::styled(mode_str, Style::default().fg(t.color(ColorRole::Success))),
            Span::styled(" | Sandbox: ", t.style_muted()),
            Span::styled(sandbox_str, Style::default().fg(t.color(ColorRole::Info))),
        ]));
        detail_lines.push(Line::raw(""));

        detail_lines.push(Line::from(Span::styled(
            "Workflow Steps:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        for (step_idx, step) in tmpl.steps.iter().enumerate() {
            detail_lines.push(Line::from(vec![
                Span::styled(format!("  {}. ", step_idx + 1), t.style_muted()),
                Span::styled(&step.title, t.style_primary().add_modifier(Modifier::BOLD)),
            ]));
            detail_lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled(&step.description, t.style_muted()),
            ]));
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            "Select a template to view details.",
            t.style_muted(),
        )));
    }

    let right_block = Block::default()
        .borders(Borders::NONE)
        .title(" Template Specification & Workflow ");
    let detail_p = Paragraph::new(detail_lines).block(right_block);
    f.render_widget(detail_p, chunks[1]);
}

fn render_goal_dashboard_modal(
    f: &mut Frame,
    goal: &Option<m31a_storage::db::Goal>,
    _scroll: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(76, 68, area);
    f.render_widget(Clear, popup_area);

    let title =
        " Long-Running Goal & Mission Dashboard  [P] Pause  [R] Resume  [C] Clear  [Esc] Close ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let mut lines = Vec::new();
    if let Some(g) = goal {
        let (status_text, status_color) = match g.status.as_str() {
            "active" => (
                "● ACTIVE (autonomous loop running)",
                t.color(ColorRole::Success),
            ),
            "paused" => (
                "⏸ PAUSED (awaiting user guidance)",
                t.color(ColorRole::Warning),
            ),
            "complete" | "completed" => (
                "✔ COMPLETED (goal objective satisfied)",
                t.color(ColorRole::Info),
            ),
            _ => ("● BLOCKED", t.color(ColorRole::Error)),
        };

        lines.push(Line::from(vec![
            Span::styled("Mission Status: ", t.style_muted()),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::raw(""));

        lines.push(Line::from(Span::styled(
            "Objective:",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", g.objective),
            t.style_primary().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));

        let budget_cap = g.token_budget.unwrap_or(200_000).max(1);
        let used = g.tokens_used;
        let pct = ((used as f64 / budget_cap as f64) * 100.0).clamp(0.0, 100.0);

        lines.push(Line::from(vec![
            Span::styled("Token Consumption: ", t.style_muted()),
            Span::styled(
                format!("{} / {} tokens ({:.1}%)", used, budget_cap, pct),
                t.style_primary(),
            ),
        ]));

        // Visual progress gauge bar
        let bar_width = 36usize;
        let filled = (((pct / 100.0) * bar_width as f64).round() as usize).min(bar_width);
        let empty = bar_width.saturating_sub(filled);
        let gauge = format!("  [{}{}]", "■".repeat(filled), " ".repeat(empty));
        let gauge_color = if pct > 85.0 {
            t.color(ColorRole::Error)
        } else if pct > 60.0 {
            t.color(ColorRole::Warning)
        } else {
            t.color(ColorRole::Success)
        };
        lines.push(Line::from(Span::styled(
            gauge,
            Style::default().fg(gauge_color),
        )));
        lines.push(Line::raw(""));

        let elapsed_mins = g.time_used_seconds / 60;
        let elapsed_secs = g.time_used_seconds % 60;
        lines.push(Line::from(vec![
            Span::styled("Time Elapsed:      ", t.style_muted()),
            Span::styled(
                format!("{}m {}s", elapsed_mins, elapsed_secs),
                t.style_primary(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Goal ID:           ", t.style_muted()),
            Span::styled(&g.goal_id, t.style_muted()),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "No active goal set for this session.",
            t.style_muted(),
        )));
        lines.push(Line::from(Span::styled(
            "To start an autonomous mission, run `/goal <objective>` in the prompt composer.",
            t.style_primary(),
        )));
    }

    let p = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(p, inner);
}

fn render_subagents_modal(
    f: &mut Frame,
    subagents: &[(String, String)],
    recent_posts: &[m31a_storage::BoardPost],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(84, 76, area);
    f.render_widget(Clear, popup_area);

    let title = format!(
        " Subagents Swarm & Command Center  {} Active  [s] Switch Tab  [k] Terminate  [Esc] Close ",
        subagents.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    // Left column: Swarm Agents List
    let mut items = Vec::new();
    if subagents.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  No child subagents active.",
            t.style_muted(),
        ))));
        items.push(ListItem::new(Line::from(Span::styled(
            "  Deploy agents using spawn_agent tool.",
            t.style_muted(),
        ))));
    } else {
        for (idx, (child_id, role)) in subagents.iter().enumerate() {
            let is_sel = idx == selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            items.push(ListItem::new(vec![
                Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(
                        format!("[{}] ", role),
                        Style::default()
                            .fg(t.color(ColorRole::Accent))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("id: {}", child_id), row_style),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        "● running in isolated sandbox",
                        Style::default().fg(t.color(ColorRole::Success)),
                    ),
                ]),
            ]));
        }
    }

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(t.border_default())
        .title(" Active Swarm Agents ");
    let agent_list = List::new(items).block(left_block);
    f.render_widget(agent_list, columns[0]);

    // Right column: Selected Agent Overview + Message Board Feed
    let mut detail_lines = Vec::new();
    if let Some((child_id, role)) = subagents.get(selected_index) {
        detail_lines.push(Line::from(vec![
            Span::styled("Agent Role:      ", t.style_muted()),
            Span::styled(role, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Thread ID:       ", t.style_muted()),
            Span::styled(child_id, t.style_secondary()),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Status:          ", t.style_muted()),
            Span::styled(
                "Active / Executing",
                Style::default()
                    .fg(t.color(ColorRole::Success))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        detail_lines.push(Line::raw(""));

        detail_lines.push(Line::from(Span::styled(
            "── Inter-Agent Message Board Feed ────────────────────",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )));

        if recent_posts.is_empty() {
            detail_lines.push(Line::from(Span::styled(
                "  (No inter-agent messages posted yet on board)",
                t.style_muted(),
            )));
        } else {
            for post in recent_posts.iter().take(6) {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!("• [{}] {}: ", post.author, post.channel),
                        t.style_secondary().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&post.body, t.style_primary()),
                ]));
            }
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            "Select a subagent on the left to inspect its status and messages.",
            t.style_muted(),
        )));
    }

    let right_block = Block::default()
        .borders(Borders::NONE)
        .title(" Agent Details & Message Board ");
    let right_p = Paragraph::new(detail_lines).block(right_block);
    f.render_widget(right_p, columns[1]);
}

fn render_turn_scrubber_modal(
    f: &mut Frame,
    turns: &[ScrubTurnItem],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 74, area);
    f.render_widget(Clear, popup_area);

    let title =
        " Esc-Backtracking Turn Scrubber  ↑↓ Navigate  Enter=Fork & Edit Prompt  Esc=Cancel ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let mut items = Vec::new();
    if turns.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  No prior conversation turns to scrub.",
            t.style_muted(),
        ))));
    } else {
        for (idx, turn) in turns.iter().enumerate() {
            let is_sel = idx == selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            let chk_str = if let Some(ref chk) = turn.checkpoint_id {
                format!(" [checkpoint: {}]", &chk[..8.min(chk.len())])
            } else {
                String::new()
            };

            let preview = if turn.user_prompt.len() > 68 {
                format!("{}...", &turn.user_prompt[..65])
            } else {
                turn.user_prompt.clone()
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(
                        format!("Turn #{} ", turn.turn_index),
                        Style::default()
                            .fg(t.color(ColorRole::Accent))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(chk_str, t.style_muted()),
                ]),
                Line::from(vec![
                    Span::raw("    User: "),
                    Span::styled(preview, row_style.add_modifier(Modifier::BOLD)),
                ]),
            ];

            if let Some(ref resp) = turn.assistant_preview {
                let resp_short = if resp.len() > 64 {
                    format!("{}...", &resp[..61])
                } else {
                    resp.clone()
                };
                lines.push(Line::from(vec![
                    Span::raw("    Resp: "),
                    Span::styled(resp_short, t.style_muted()),
                ]));
            }

            lines.push(Line::raw(""));
            items.push(ListItem::new(lines));
        }
    }

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    f.render_widget(list, inner);
}

#[allow(clippy::too_many_arguments)]
fn render_context_manager_modal(
    f: &mut Frame,
    app: &App,
    pinned_items: &[m31a_core::DynamicContextItem],
    world_sections: &[(String, String)],
    selected_index: usize,
    active_tab: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(82, 78, area);
    f.render_widget(Clear, popup_area);

    let title = " Context Window & Pinned Files Manager  [Tab] Switch View  ↑↓ Select  Esc=Close ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(inner);

    // Tab headers
    let tab_labels = [
        "1. Context Overview",
        "2. Pinned Files",
        "3. World State Sections",
    ];
    let mut tab_spans = Vec::new();
    for (idx, label) in tab_labels.iter().enumerate() {
        let is_active = idx == active_tab;
        let tab_style = if is_active {
            Style::default()
                .fg(t.color(ColorRole::Surface0))
                .bg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            t.style_muted()
        };
        tab_spans.push(Span::styled(format!(" {} ", label), tab_style));
        tab_spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

    match active_tab {
        0 => {
            // Overview
            let context_limit = 131_072usize;
            let tokens_used = app.tokens_used as usize;
            let pct = ((tokens_used as f64 / context_limit as f64) * 100.0).min(100.0);
            let pinned_tokens: usize = pinned_items.iter().map(|p| p.tokens).sum();

            let mut lines = Vec::new();
            lines.push(Line::from(vec![
                Span::styled("Active Model:      ", t.style_muted()),
                Span::styled(
                    &app.active_model,
                    t.style_primary().add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Context Window:    ", t.style_muted()),
                Span::styled(
                    format!("{} tokens ({:.1}% consumed)", context_limit, pct),
                    t.style_primary(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Tokens Used:       ", t.style_muted()),
                Span::styled(format!("{} tokens", tokens_used), t.style_primary()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Pinned Files Cost: ", t.style_muted()),
                Span::styled(format!("~{} tokens", pinned_tokens), t.style_secondary()),
            ]));
            lines.push(Line::raw(""));

            // Capacity bar
            let bar_width = 40usize;
            let filled = (((pct / 100.0) * bar_width as f64).round() as usize).min(bar_width);
            let empty = bar_width.saturating_sub(filled);
            lines.push(Line::from(Span::styled(
                format!("Capacity: [{}{}]", "■".repeat(filled), " ".repeat(empty)),
                Style::default().fg(if pct > 80.0 {
                    t.color(ColorRole::Error)
                } else {
                    t.color(ColorRole::Success)
                }),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "Commands: /context pin <path> | /context unpin <path> | /compact",
                t.style_muted(),
            )));

            let p = Paragraph::new(lines);
            f.render_widget(p, chunks[1]);
        }
        1 => {
            // Pinned Files
            let mut items = Vec::new();
            if pinned_items.is_empty() {
                items.push(ListItem::new(Line::from(Span::styled(
                    "  No files currently pinned in dynamic context.",
                    t.style_muted(),
                ))));
                items.push(ListItem::new(Line::from(Span::styled(
                    "  Use `/context pin <file-path>` to pin relevant documentation or types.",
                    t.style_muted(),
                ))));
            } else {
                for (idx, p) in pinned_items.iter().enumerate() {
                    let is_sel = idx == selected_index;
                    let marker = if is_sel { "▸ " } else { "  " };
                    let row_style = if is_sel {
                        t.style_selected()
                    } else {
                        t.style_primary()
                    };

                    items.push(ListItem::new(Line::from(vec![
                        Span::styled(marker, row_style),
                        Span::styled(&p.path, row_style.add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" (~{} tokens)", p.tokens), t.style_muted()),
                    ])));
                }
            }
            let list = List::new(items).block(Block::default().borders(Borders::NONE));
            f.render_widget(list, chunks[1]);
        }
        _ => {
            // World State Sections
            let mut items = Vec::new();
            for (idx, (sec_id, digest)) in world_sections.iter().enumerate() {
                let is_sel = idx == selected_index;
                let marker = if is_sel { "▸ " } else { "  " };
                let row_style = if is_sel {
                    t.style_selected()
                } else {
                    t.style_primary()
                };
                let short_hash: String = digest.chars().take(8).collect();

                items.push(ListItem::new(Line::from(vec![
                    Span::styled(marker, row_style),
                    Span::styled(
                        format!("{:<22} ", sec_id),
                        row_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("sha1:{}", short_hash), t.style_muted()),
                ])));
            }
            let list = List::new(items).block(Block::default().borders(Borders::NONE));
            f.render_widget(list, chunks[1]);
        }
    }
}

fn render_persona_modal(
    f: &mut Frame,
    personas: &[crate::modals::PersonaItem],
    selected_index: usize,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(76, 74, area);
    f.render_widget(Clear, popup_area);

    let title = " Select Engineering Persona / Workstyle  ↑↓ Select  Enter=Activate  Esc=Close ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![Span::styled(
            title,
            t.style_secondary().add_modifier(Modifier::BOLD),
        )]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let mut items = Vec::new();
    for (idx, p) in personas.iter().enumerate() {
        let is_sel = idx == selected_index;
        let marker = if is_sel { "▸ " } else { "  " };
        let row_style = if is_sel {
            t.style_selected()
        } else {
            t.style_primary()
        };

        items.push(ListItem::new(vec![
            Line::from(vec![
                Span::styled(marker, row_style),
                Span::styled(format!("{} ", p.icon.render(t)), Style::default()),
                Span::styled(&p.name, row_style.add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(&p.description, t.style_muted()),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("\"{}\"", p.prompt_directive),
                    Style::default()
                        .fg(t.color(ColorRole::TextSecondary))
                        .add_modifier(Modifier::ITALIC),
                ),
            ]),
            Line::raw(""),
        ]));
    }

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    f.render_widget(list, inner);
}

fn render_code_mode_scratchpad(
    f: &mut Frame,
    sp: &crate::modals::CodeModeScratchpadState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(90, 85, area);
    f.render_widget(Clear, popup_area);

    let title = if popup_area.width < 85 {
        " Code Mode Scratchpad | [F5] Run | [Tab] Focus | [Esc] Exit "
    } else {
        " V8 Code Mode Scratchpad | [F5/Ctrl+Enter] Run | [Tab] Switch Focus | [1-3] Tabs | [Esc] Exit "
    };

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(" ⚡ ", Style::default().fg(t.color(ColorRole::Warning))),
            Span::styled(title, t.style_secondary().add_modifier(Modifier::BOLD)),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = main_block.inner(popup_area);
    f.render_widget(main_block, popup_area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let is_editor_focused = sp.focus == crate::modals::ScratchpadFocus::Editor;
    let editor_border_style = if is_editor_focused {
        t.border_focus()
    } else {
        t.border_default()
    };

    let editor_block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(Line::from(vec![
            Span::styled(
                " TypeScript/JavaScript Code ",
                if is_editor_focused {
                    Style::default()
                        .fg(t.color(ColorRole::Accent))
                        .add_modifier(Modifier::BOLD)
                } else {
                    t.style_muted()
                },
            ),
            Span::styled(
                format!("Ln {}, Col {} ", sp.cursor_line + 1, sp.cursor_col + 1),
                t.style_muted(),
            ),
        ]))
        .border_style(editor_border_style)
        .style(t.style_surface0());

    let editor_inner = editor_block.inner(cols[0]);
    f.render_widget(editor_block, cols[0]);

    let lines: Vec<&str> = sp.code.lines().collect();
    let mut editor_items = Vec::new();
    let display_lines = if lines.is_empty() { vec![""] } else { lines };

    for (line_idx, line) in display_lines.iter().enumerate() {
        let is_curr_line = line_idx == sp.cursor_line && is_editor_focused;
        let line_num_str = format!("{:>3} │ ", line_idx + 1);
        let num_style = if is_curr_line {
            Style::default()
                .fg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            t.style_muted()
        };

        let mut spans = vec![Span::styled(line_num_str, num_style)];

        if is_curr_line {
            let chars: Vec<char> = line.chars().collect();
            let col = sp.cursor_col.min(chars.len());
            let before: String = chars.iter().take(col).collect();
            let cursor_char = if col < chars.len() {
                chars[col].to_string()
            } else {
                " ".to_string()
            };
            let after: String = chars.iter().skip(col + 1).collect();

            spans.push(Span::styled(before, t.style_primary()));
            spans.push(Span::styled(
                cursor_char,
                Style::default()
                    .bg(t.color(ColorRole::Accent))
                    .fg(t.color(ColorRole::Surface0)),
            ));
            spans.push(Span::styled(after, t.style_primary()));
        } else {
            let highlighted = crate::markdown_syntax::highlight_code_line(line, Some("ts"));
            for span in highlighted {
                spans.push(span);
            }
        }

        let bg_style = if is_curr_line {
            Style::default().bg(t.color(ColorRole::Surface1))
        } else {
            Style::default()
        };

        editor_items.push(ListItem::new(Line::from(spans)).style(bg_style));
    }

    let editor_list = List::new(editor_items).block(Block::default().borders(Borders::NONE));
    f.render_widget(editor_list, editor_inner);

    let is_viewer_focused = sp.focus == crate::modals::ScratchpadFocus::Viewer;
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(5)])
        .split(cols[1]);

    let tab_titles = [
        if sp.active_tab == 0 {
            " [1] Output (Active) "
        } else {
            " [1] Output "
        },
        if sp.active_tab == 1 {
            " [2] Ambient .d.ts (Active) "
        } else {
            " [2] Ambient .d.ts "
        },
        if sp.active_tab == 2 {
            " [3] Budgets & Engine (Active) "
        } else {
            " [3] Budgets & Engine "
        },
    ];
    let mut tab_spans = Vec::new();
    for (idx, title) in tab_titles.iter().enumerate() {
        let is_sel = idx == sp.active_tab;
        let tab_style = if is_sel {
            Style::default()
                .fg(t.color(ColorRole::Accent))
                .add_modifier(Modifier::BOLD)
        } else {
            t.style_muted()
        };
        tab_spans.push(Span::styled(*title, tab_style));
        if idx + 1 < tab_titles.len() {
            tab_spans.push(Span::raw(" | "));
        }
    }
    f.render_widget(Paragraph::new(Line::from(tab_spans)), right_chunks[0]);

    let viewer_border_style = if is_viewer_focused {
        t.border_focus()
    } else {
        t.border_default()
    };

    match sp.active_tab {
        0 => {
            let mut title_spans = vec![Span::raw(" Execution Output ")];
            if sp.is_evaluating {
                title_spans.push(Span::styled(
                    " [EVALUATING...] ",
                    Style::default()
                        .fg(t.color(ColorRole::Warning))
                        .add_modifier(Modifier::BOLD),
                ));
            }
            let out_block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(Line::from(title_spans))
                .border_style(viewer_border_style)
                .style(t.style_surface0());

            let out_inner = out_block.inner(right_chunks[1]);
            f.render_widget(out_block, right_chunks[1]);

            let mut out_items = Vec::new();
            for line in sp.output_lines.iter().skip(sp.scroll_output) {
                let style = if line.starts_with("// [Evaluation Error]") || line.contains("Error:")
                {
                    Style::default().fg(t.color(ColorRole::Error))
                } else if line.starts_with("// [Evaluation completed") {
                    Style::default()
                        .fg(t.color(ColorRole::Success))
                        .add_modifier(Modifier::BOLD)
                } else if line.starts_with("//") {
                    t.style_muted()
                } else {
                    t.style_primary()
                };
                out_items.push(ListItem::new(Line::from(Span::styled(line.clone(), style))));
            }
            let out_list = List::new(out_items).block(Block::default().borders(Borders::NONE));
            f.render_widget(out_list, out_inner);
        }
        1 => {
            let dts_block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(Line::from(vec![
                    Span::raw(" Ambient TypeScript Declarations (m31a_ambient.d.ts) "),
                    Span::styled(
                        format!("({} lines) ", sp.ambient_dts_lines.len()),
                        t.style_muted(),
                    ),
                ]))
                .border_style(viewer_border_style)
                .style(t.style_surface0());

            let dts_inner = dts_block.inner(right_chunks[1]);
            f.render_widget(dts_block, right_chunks[1]);

            let mut dts_items = Vec::new();
            for line in sp.ambient_dts_lines.iter().skip(sp.scroll_dts) {
                let style = if line.starts_with("/**")
                    || line.starts_with(" *")
                    || line.starts_with(" */")
                {
                    t.style_muted()
                } else if line.contains("declare namespace") || line.contains("declare function") {
                    Style::default()
                        .fg(t.color(ColorRole::CodeKeyword))
                        .add_modifier(Modifier::BOLD)
                } else if line.contains("type ") || line.contains("interface ") {
                    Style::default().fg(t.color(ColorRole::CodeType))
                } else {
                    t.style_primary()
                };
                dts_items.push(ListItem::new(Line::from(Span::styled(line.clone(), style))));
            }
            let dts_list = List::new(dts_items).block(Block::default().borders(Borders::NONE));
            f.render_widget(dts_list, dts_inner);
        }
        _ => {
            let budget_block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" Code Mode Isolation & Resource Constraints ")
                .border_style(viewer_border_style)
                .style(t.style_surface0());

            let b_inner = budget_block.inner(right_chunks[1]);
            f.render_widget(budget_block, right_chunks[1]);

            let text = vec![
                Line::from(vec![
                    Span::styled("Execution Runtime:      ", t.style_muted()),
                    Span::styled(
                        "m31a-code-mode (Confined JS/TS Isolate)",
                        t.style_primary().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Memory Ceiling:         ", t.style_muted()),
                    Span::styled(
                        "32 MiB strict heap quota",
                        Style::default().fg(t.color(ColorRole::Success)),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Yield Timeout:          ", t.style_muted()),
                    Span::styled(
                        format!("{} ms per batch iteration", sp.yield_ms),
                        t.style_primary(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Token Output Budget:    ", t.style_muted()),
                    Span::styled(
                        format!("{} tokens maximum", sp.max_tokens),
                        t.style_primary(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Ambient Globals:        ", t.style_muted()),
                    Span::styled(
                        "print(), yieldToUser(), store, tools.*",
                        Style::default().fg(t.color(ColorRole::Accent)),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Safety Boundary:        ", t.style_muted()),
                    Span::styled(
                        "Hermetic sandbox write enforcement",
                        Style::default().fg(t.color(ColorRole::Warning)),
                    ),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        "Tip: ",
                        Style::default()
                            .fg(t.color(ColorRole::Info))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Code Mode executes multi-step workflows in a single LLM turn,",
                        t.style_muted(),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(
                        "drastically reducing token spend and network latency.",
                        t.style_muted(),
                    ),
                ]),
            ];

            let p = Paragraph::new(text);
            f.render_widget(p, b_inner);
        }
    }
}

fn render_rollout_journal(
    f: &mut Frame,
    journal: &crate::modals::RolloutJournalState,
    area: Rect,
    t: &Theme,
) {
    let popup_area = centered_rect(88, 80, area);
    f.render_widget(Clear, popup_area);

    let title = " Rollout Journal & Event Timeline | [↑/↓] Select | [Enter/Space] Toggle JSON | [a/p/t/c] Filter | [Esc] Exit ";

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border.modal)
        .title(Line::from(vec![
            Span::styled(
                format!(" {} ", crate::icons::Icon::FileText.render(t)),
                Style::default(),
            ),
            Span::styled(title, t.style_secondary().add_modifier(Modifier::BOLD)),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface1());

    let inner = main_block.inner(popup_area);
    f.render_widget(main_block, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(5)])
        .split(inner);

    let counts = (
        journal.events.len(),
        journal
            .events
            .iter()
            .filter(|e| e.category == "PROMPT")
            .count(),
        journal
            .events
            .iter()
            .filter(|e| e.category == "TOOL" || e.category == "TOOL_RES")
            .count(),
        journal
            .events
            .iter()
            .filter(|e| e.category == "CHECKPOINT")
            .count(),
    );

    let filter_spans = vec![
        Span::styled(
            format!(" [a] All ({}) ", counts.0),
            if journal.filter == "all" {
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                t.style_muted()
            },
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" [p] Prompts ({}) ", counts.1),
            if journal.filter == "prompts" {
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                t.style_muted()
            },
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" [t] Tools ({}) ", counts.2),
            if journal.filter == "tools" {
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                t.style_muted()
            },
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" [c] Checkpoints ({}) ", counts.3),
            if journal.filter == "checkpoints" {
                Style::default()
                    .fg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                t.style_muted()
            },
        ),
    ];
    f.render_widget(Paragraph::new(Line::from(filter_spans)), layout[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[1]);

    let filtered_indices = journal.filtered_indices();

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Event Sequence ")
        .border_style(t.border_default())
        .style(t.style_surface0());

    let list_inner = list_block.inner(cols[0]);
    f.render_widget(list_block, cols[0]);

    let mut list_items = Vec::new();
    for (row_idx, &ev_idx) in filtered_indices.iter().enumerate() {
        if let Some(ev) = journal.events.get(ev_idx) {
            let is_sel = row_idx == journal.selected_index;
            let marker = if is_sel { "▸ " } else { "  " };
            let (badge_style, badge_txt) = match ev.category.as_str() {
                "PROMPT" => (
                    Style::default()
                        .fg(t.color(ColorRole::Accent))
                        .add_modifier(Modifier::BOLD),
                    "[PROMPT]",
                ),
                "ASSISTANT" => (Style::default().fg(t.color(ColorRole::Success)), "[ASSIST]"),
                "TOOL" | "TOOL_RES" => {
                    (Style::default().fg(t.color(ColorRole::Warning)), "[TOOL]  ")
                }
                "CHECKPOINT" => (Style::default().fg(t.color(ColorRole::Info)), "[CHKPT] "),
                "METRIC" => (
                    Style::default().fg(t.color(ColorRole::CodeKeyword)),
                    "[METRIC]",
                ),
                _ => (t.style_muted(), "[NOTICE]"),
            };

            let row_style = if is_sel {
                t.style_selected()
            } else {
                t.style_primary()
            };

            list_items.push(ListItem::new(Line::from(vec![
                Span::styled(marker, row_style),
                Span::styled(format!("{} ", ev.timestamp), t.style_muted()),
                Span::styled(format!("{} ", badge_txt), badge_style),
                Span::styled(
                    &ev.title,
                    row_style.add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
            ])));
        }
    }

    let event_list = List::new(list_items).block(Block::default().borders(Borders::NONE));
    f.render_widget(event_list, list_inner);

    let selected_event = filtered_indices
        .get(journal.selected_index)
        .and_then(|&idx| journal.events.get(idx));

    let view_mode_txt = if journal.view_json {
        " (Raw JSON View) "
    } else {
        " (Structured View) "
    };
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(Line::from(vec![
            Span::raw(" Event Payload Inspector "),
            Span::styled(
                view_mode_txt,
                Style::default().fg(t.color(ColorRole::Accent)),
            ),
        ]))
        .border_style(t.border_focus())
        .style(t.style_surface0());

    let detail_inner = detail_block.inner(cols[1]);
    f.render_widget(detail_block, cols[1]);

    if let Some(ev) = selected_event {
        let mut detail_lines = Vec::new();
        detail_lines.push(Line::from(vec![
            Span::styled("Event ID:   ", t.style_muted()),
            Span::styled(
                format!("#{}", ev.id),
                t.style_primary().add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Timestamp: ", t.style_muted()),
            Span::styled(&ev.timestamp, t.style_primary()),
            Span::raw("    "),
            Span::styled("Category: ", t.style_muted()),
            Span::styled(
                &ev.category,
                Style::default().fg(t.color(ColorRole::Accent)),
            ),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("Summary:    ", t.style_muted()),
            Span::styled(&ev.title, t.style_primary().add_modifier(Modifier::BOLD)),
        ]));
        if let Some(tokens) = ev.tokens {
            detail_lines.push(Line::from(vec![
                Span::styled("Tokens:     ", t.style_muted()),
                Span::styled(
                    format!("{} tokens", tokens),
                    Style::default().fg(t.color(ColorRole::Success)),
                ),
            ]));
        }
        detail_lines.push(Line::styled(
            "────────────────────────────────────────────────────────",
            t.style_muted(),
        ));

        let text_lines: Vec<&str> = ev.detail.lines().collect();
        for line in text_lines.iter().skip(journal.scroll_detail) {
            let style = if journal.view_json {
                if line.contains("\":") {
                    Style::default().fg(t.color(ColorRole::CodeKeyword))
                } else if line.contains("\"") {
                    Style::default().fg(t.color(ColorRole::CodeString))
                } else {
                    t.style_primary()
                }
            } else {
                t.style_primary()
            };
            detail_lines.push(Line::from(Span::styled(line.to_string(), style)));
        }

        let p = Paragraph::new(detail_lines);
        f.render_widget(p, detail_inner);
    } else {
        let empty_p = Paragraph::new(Line::styled("No event selected.", t.style_muted()));
        f.render_widget(empty_p, detail_inner);
    }
}
