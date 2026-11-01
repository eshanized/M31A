//! Rich mouse support, gesture tracking, and contextual click/scroll routing.
//!
//! Rich terminal mouse event handling, gesture tracking, modal dismissal, and contextual click/scroll routing:
//! - Mouse wheel scrolling with configurable speed multiplier and acceleration curve.
//! - Context-aware scrolling for modals (DiffViewer, ProcessViewer, Help, Settings, Model Picker).
//! - Backdrop dismiss on click outside modal dialogs.
//! - Interactive clicking for dialog choices (Approval, Settings tabs/items, Model picker, Command palette, Sessions).
//! - Header badge clicks (toggle Mode BUILD/PLAN, switch Model).
//! - Footer shortcut clicks (Palette, Mode, Models, Diff, Resume, Help).
//! - Click on transcript unpinned indicator to jump to bottom.
//! - Click on transcript ToolCall / Thinking headers to toggle collapsed state.
//! - Drag-to-select text and automatic copy-on-select integration.
//! - Scrollbar track and arrow clicking for fast vertical navigation.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::widgets::Block;

use crate::app::App;
use crate::bottom_pane::BottomPaneMode;
use crate::modals::ModalState;
use crate::transcript::{NoticeLevel, TranscriptItem};

/// Tracked mouse drag selection across terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseSelection {
    pub start: (u16, u16),   // (col, row)
    pub current: (u16, u16), // (col, row)
    pub active: bool,
}

impl MouseSelection {
    pub fn new(col: u16, row: u16) -> Self {
        Self {
            start: (col, row),
            current: (col, row),
            active: false,
        }
    }
}

/// Dispatches mouse events according to current TUI view state and layout.
pub fn handle_mouse_event(app: &mut App, mouse: MouseEvent, screen_area: Rect) {
    if !app.config.tui.mouse_enabled {
        return;
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            handle_mouse_scroll(app, true, mouse.column, mouse.row, screen_area);
        }
        MouseEventKind::ScrollDown => {
            handle_mouse_scroll(app, false, mouse.column, mouse.row, screen_area);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click_down(app, mouse.column, mouse.row, screen_area);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_left_click_drag(app, mouse.column, mouse.row, screen_area);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            handle_left_click_up(app, mouse.column, mouse.row, screen_area);
        }
        MouseEventKind::Down(MouseButton::Middle)
            // Middle-click closes active modal immediately if open
            if app.has_active_modal() => {
                app.pop_modal();
            }
        _ => {}
    }
}

fn handle_mouse_scroll(app: &mut App, is_up: bool, _col: u16, _row: u16, _screen_area: Rect) {
    let speed_mult = app.config.tui.mouse_scroll_speed;
    let delta = (3.0 * speed_mult).round().max(1.0) as usize;

    // 1. Context-aware scrolling if modal is active
    if let Some(modal) = app.pop_modal() {
        let updated = match modal {
            ModalState::DiffViewer {
                diff_text,
                mut scroll,
            } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else {
                    scroll = scroll.saturating_add(delta);
                }
                ModalState::DiffViewer { diff_text, scroll }
            }
            ModalState::QuickDiffPreview {
                diff_text,
                return_to_approval,
                mut scroll,
            } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else {
                    scroll = scroll.saturating_add(delta);
                }
                ModalState::QuickDiffPreview {
                    diff_text,
                    return_to_approval,
                    scroll,
                }
            }
            ModalState::ProcessViewer {
                process_id,
                label,
                lines,
                mut scroll,
            } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else if scroll + delta < lines.len() {
                    scroll += delta;
                }
                ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                }
            }
            ModalState::HelpModal { mut scroll } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else {
                    scroll = scroll.saturating_add(delta);
                }
                ModalState::HelpModal { scroll }
            }
            ModalState::CriticalPathModal {
                durations,
                completed_intervals,
                mut scroll,
            } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else if scroll + delta < completed_intervals.len() {
                    scroll += delta;
                }
                ModalState::CriticalPathModal {
                    durations,
                    completed_intervals,
                    scroll,
                }
            }
            ModalState::InteractiveDiff(mut s) => {
                if is_up {
                    s.scroll_up(delta);
                } else {
                    s.scroll_down(delta);
                }
                ModalState::InteractiveDiff(s)
            }
            ModalState::ModelPicker { mut state } => {
                if is_up {
                    state.select_prev();
                } else {
                    state.select_next();
                }
                ModalState::ModelPicker { state }
            }
            ModalState::CommandPalette {
                query,
                mut selected_index,
                items,
                filtered_indices,
                mut scroll_offset,
            } => {
                let visible_items = 12usize;
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                    if selected_index < scroll_offset {
                        scroll_offset = selected_index;
                    }
                } else if !filtered_indices.is_empty()
                    && selected_index + 1 < filtered_indices.len()
                {
                    selected_index += 1;
                    if selected_index >= scroll_offset + visible_items {
                        scroll_offset =
                            selected_index.saturating_sub(visible_items.saturating_sub(1));
                    }
                }
                ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                    scroll_offset,
                }
            }
            ModalState::ReasoningEffort { mut state } => {
                if is_up {
                    state.prev();
                } else {
                    state.next();
                }
                ModalState::ReasoningEffort { state }
            }
            ModalState::SettingsModal {
                active_tab,
                mut selected_index,
            } => {
                let max = settings_items_count(active_tab);
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < max {
                    selected_index += 1;
                }
                ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                }
            }
            ModalState::ResumePicker(mut picker) => {
                if is_up {
                    picker.move_up();
                } else {
                    picker.move_down();
                }
                ModalState::ResumePicker(picker)
            }
            ModalState::ExperimentalFeatures {
                items,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < items.len() {
                    selected_index += 1;
                }
                ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                }
            }
            ModalState::ProcessesModal {
                processes,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < processes.len() {
                    selected_index += 1;
                }
                ModalState::ProcessesModal {
                    processes,
                    selected_index,
                }
            }
            ModalState::CheckpointsModal {
                checkpoints,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < checkpoints.len() {
                    selected_index += 1;
                }
                ModalState::CheckpointsModal {
                    checkpoints,
                    selected_index,
                }
            }
            ModalState::ActiveLocksModal {
                locks,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < locks.len() {
                    selected_index += 1;
                }
                ModalState::ActiveLocksModal {
                    locks,
                    selected_index,
                }
            }
            ModalState::SymbolOutlineModal {
                file_path,
                symbols,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if selected_index + 1 < symbols.len() {
                    selected_index += 1;
                }
                ModalState::SymbolOutlineModal {
                    file_path,
                    symbols,
                    selected_index,
                }
            }
            ModalState::McpHubModal {
                servers,
                mut selected_index,
                view_tools,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !servers.is_empty() && selected_index + 1 < servers.len() {
                    selected_index += 1;
                }
                ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                }
            }
            ModalState::SkillsModal {
                skills,
                mut selected_index,
                search_query,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !skills.is_empty() && selected_index + 1 < skills.len() {
                    selected_index += 1;
                }
                ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                }
            }
            ModalState::TemplatesModal {
                templates,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !templates.is_empty() && selected_index + 1 < templates.len() {
                    selected_index += 1;
                }
                ModalState::TemplatesModal {
                    templates,
                    selected_index,
                }
            }
            ModalState::GoalDashboardModal { goal, mut scroll } => {
                if is_up {
                    scroll = scroll.saturating_sub(delta);
                } else {
                    scroll = scroll.saturating_add(delta);
                }
                ModalState::GoalDashboardModal { goal, scroll }
            }
            ModalState::SubagentsModal {
                subagents,
                mut selected_index,
                recent_posts,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !subagents.is_empty() && selected_index + 1 < subagents.len() {
                    selected_index += 1;
                }
                ModalState::SubagentsModal {
                    subagents,
                    selected_index,
                    recent_posts,
                }
            }
            ModalState::TurnScrubber {
                turns,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !turns.is_empty() && selected_index + 1 < turns.len() {
                    selected_index += 1;
                }
                ModalState::TurnScrubber {
                    turns,
                    selected_index,
                }
            }
            ModalState::ContextManagerModal {
                pinned_items,
                world_sections,
                mut selected_index,
                active_tab,
            } => {
                let limit = if active_tab == 0 {
                    pinned_items.len()
                } else {
                    world_sections.len()
                };
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if limit > 0 && selected_index + 1 < limit {
                    selected_index += 1;
                }
                ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                }
            }
            ModalState::PersonaModal {
                personas,
                mut selected_index,
            } => {
                if is_up {
                    selected_index = selected_index.saturating_sub(1);
                } else if !personas.is_empty() && selected_index + 1 < personas.len() {
                    selected_index += 1;
                }
                ModalState::PersonaModal {
                    personas,
                    selected_index,
                }
            }
            ModalState::CodeModeScratchpad(mut sp) => {
                if is_up {
                    if sp.active_tab == 0 {
                        sp.scroll_output = sp.scroll_output.saturating_sub(delta);
                    } else if sp.active_tab == 1 {
                        sp.scroll_dts = sp.scroll_dts.saturating_sub(delta);
                    }
                } else {
                    if sp.active_tab == 0 {
                        sp.scroll_output = sp.scroll_output.saturating_add(delta);
                    } else if sp.active_tab == 1 {
                        sp.scroll_dts = sp.scroll_dts.saturating_add(delta);
                    }
                }
                ModalState::CodeModeScratchpad(sp)
            }
            ModalState::RolloutJournal(mut journal) => {
                let filtered_len = journal.filtered_indices().len();
                if is_up {
                    journal.selected_index = journal.selected_index.saturating_sub(1);
                } else if filtered_len > 0 && journal.selected_index + 1 < filtered_len {
                    journal.selected_index += 1;
                }
                ModalState::RolloutJournal(journal)
            }
            other => other,
        };
        app.push_modal(updated);
        return;
    }

    // 2. Check if bottom pane popups are active
    match &mut app.bottom_pane.mode {
        BottomPaneMode::SlashCommandPopup {
            candidates,
            selected_index,
        } => {
            if is_up {
                *selected_index = selected_index.saturating_sub(1);
            } else if !candidates.is_empty() && *selected_index + 1 < candidates.len() {
                *selected_index += 1;
            }
            return;
        }
        BottomPaneMode::ModelSelectorPopup { .. } => {
            if let Some(ref mut ms) = app.bottom_pane.model_selector {
                if is_up {
                    ms.select_prev();
                } else {
                    ms.select_next();
                }
            }
            return;
        }
        BottomPaneMode::FileMentionPopup {
            candidates,
            selected_index,
            ..
        }
        | BottomPaneMode::SkillMentionPopup {
            candidates,
            selected_index,
            ..
        } => {
            if is_up {
                *selected_index = selected_index.saturating_sub(1);
            } else if !candidates.is_empty() && *selected_index + 1 < candidates.len() {
                *selected_index += 1;
            }
            return;
        }
        _ => {}
    }

    // Screen::Models scrolling
    if app.current_screen == crate::screens::Screen::Models
        && let Some(ref mut ms) = app.models_screen_state
    {
        if is_up {
            ms.select_prev();
        } else {
            ms.select_next();
        }
        app.screen_selected_index = ms.selected_index;
        return;
    }

    // 3. Otherwise: default to scrolling conversation transcript
    if is_up {
        app.scroll_up(delta);
    } else {
        app.scroll_down(delta);
    }
}

fn handle_left_click_down(app: &mut App, col: u16, row: u16, screen_area: Rect) {
    let click_pos = Position::new(col, row);

    // 1. Modal Dialog Interactions
    if let Some(modal) = app.pop_modal() {
        let popup_area = modal.calculate_rect(screen_area);

        // Click OUTSIDE modal
        if !popup_area.contains(click_pos) {
            match modal {
                ModalState::ApprovalDialog { .. } | ModalState::DeclineFeedbackInput { .. } => {
                    // Do not pop approval dialog on backdrop click! Retain pending approval.
                    app.push_modal(modal);
                }
                _ => {
                    // Standard modal backdrop dismissal
                }
            }
            return;
        }

        // Click INSIDE modal -> interactive component routing
        match modal {
            ModalState::ApprovalDialog {
                approval,
                selected_option,
            } => {
                let action = crate::modals::approval_action_rects(screen_area, &approval)
                    .into_iter()
                    .find(|(rect, _)| rect.contains(click_pos))
                    .map(|(_, action)| action);
                if let Some(action) = action {
                    let handled = app.resolve_approval_action(approval.clone(), action);
                    if !handled && !app.has_active_modal() {
                        app.push_modal(ModalState::ApprovalDialog {
                            approval,
                            selected_option,
                        });
                    }
                    return;
                }

                // Clicks outside the action rows keep the approval pending.
                app.push_modal(ModalState::ApprovalDialog {
                    approval,
                    selected_option,
                });
            }
            ModalState::SettingsModal {
                mut active_tab,
                mut selected_index,
            } => {
                let inner = Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .inner(popup_area);
                if row == inner.y {
                    // Click on Tab bar
                    let rel_x = col.saturating_sub(inner.x);
                    if rel_x < 15 {
                        active_tab = 0;
                    } else if rel_x < 31 {
                        active_tab = 1;
                    } else if rel_x < 52 {
                        active_tab = 2;
                    } else {
                        active_tab = 3;
                    }
                    selected_index = 0;
                } else if row >= inner.y + 2 && row < inner.y + inner.height.saturating_sub(1) {
                    let clicked_item = ((row - (inner.y + 2)) / 2) as usize;
                    let max_items = settings_items_count(active_tab);
                    if clicked_item < max_items {
                        if clicked_item == selected_index {
                            toggle_or_cycle_setting(app, active_tab, selected_index);
                        } else {
                            selected_index = clicked_item;
                        }
                    }
                }
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            ModalState::ModelPicker { mut state } => {
                let geometry = crate::model_selector::ModelSelectorGeometry::new(popup_area);
                state.set_viewport_height(geometry.inner_height());
                if let Some(rel_row) = geometry.row_to_visible_index(col, row) {
                    let clicked_idx = state.scroll_offset + rel_row;
                    if clicked_idx < state.filtered_indices.len() {
                        let model_idx = state.filtered_indices[clicked_idx];
                        let slug = state.models[model_idx].slug.clone();
                        match app.switch_model(&slug) {
                            Ok(_) => {
                                return;
                            }
                            Err(e) => {
                                app.provider_status_message =
                                    Some(format!("Failed to activate model: {e}"));
                                app.push_modal(ModalState::ModelPicker { state });
                                return;
                            }
                        }
                    }
                }
                app.push_modal(ModalState::ModelPicker { state });
            }
            ModalState::CommandPalette {
                query,
                selected_index,
                scroll_offset,
                items,
                filtered_indices,
            } => {
                let list_start_y = popup_area.y + 5;
                let list_end_y = popup_area.y + popup_area.height.saturating_sub(2);
                let list_max_x = if popup_area.width >= 70 {
                    popup_area.x + (popup_area.width * 58 / 100)
                } else {
                    popup_area.x + popup_area.width
                };
                if row >= list_start_y && row < list_end_y && col <= list_max_x {
                    let clicked_offset = (row - list_start_y) as usize;
                    let clicked_idx = scroll_offset + clicked_offset;
                    if clicked_idx < filtered_indices.len()
                        && let Some(&item_idx) = filtered_indices.get(clicked_idx)
                        && let Some(item) = items.get(item_idx)
                    {
                        let cmd_id = item.id.clone();
                        crate::terminal::execute_palette_action(app, &cmd_id);
                        return;
                    }
                }
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    scroll_offset,
                    items,
                    filtered_indices,
                });
            }
            ModalState::ReasoningEffort { state } => {
                let options_start_y = popup_area.y + 3;
                if row >= options_start_y && row < options_start_y + state.options.len() as u16 {
                    let clicked_idx = (row - options_start_y) as usize;
                    if let Some(opt) = state.options.get(clicked_idx) {
                        let effort_opt = match opt.id {
                            "none" => None,
                            other => Some(other.to_string()),
                        };
                        app.apply_reasoning_effort(effort_opt);
                        return;
                    }
                }
                app.push_modal(ModalState::ReasoningEffort { state });
            }
            ModalState::ExperimentalFeatures {
                mut items,
                selected_index,
            } => {
                if row > popup_area.y && row < popup_area.y + popup_area.height.saturating_sub(1) {
                    let clicked_idx = ((row - (popup_area.y + 1)) / 2) as usize;
                    if let Some((key, _name, _desc, enabled)) = items.get_mut(clicked_idx) {
                        *enabled = !*enabled;
                        app.config
                            .features
                            .insert(key.as_str().to_string(), *enabled);
                        let _ = app.config.save();
                    }
                }
                app.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                });
            }
            ModalState::ResumePicker(mut picker) => {
                let inner = Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .inner(popup_area);
                if col < popup_area.x + popup_area.width / 2 && row >= inner.y + 3 {
                    let clicked_idx = (row - (inner.y + 3)) as usize;
                    if clicked_idx < picker.filtered_indices.len() {
                        picker.selected_index = clicked_idx;
                        if let Some(thread) = picker.selected_thread().cloned() {
                            if let Err(e) = app.resume_session(&thread.id) {
                                app.provider_status_message =
                                    Some(format!("Failed to resume session: {e}"));
                                app.push_modal(ModalState::ResumePicker(picker));
                            }
                            return;
                        }
                    }
                }
                app.push_modal(ModalState::ResumePicker(picker));
            }
            ModalState::RevertConfirmModal {
                checkpoint,
                selected_option,
            } => {
                let bottom_y = popup_area.y + popup_area.height.saturating_sub(2);
                if row >= bottom_y.saturating_sub(1) {
                    let rel_x = col.saturating_sub(popup_area.x);
                    if rel_x < popup_area.width / 2 {
                        // Revert
                        let _ = app.restore_checkpoint_action(&checkpoint);
                        return;
                    } else {
                        // Cancel
                        return;
                    }
                }
                app.push_modal(ModalState::RevertConfirmModal {
                    checkpoint,
                    selected_option,
                });
            }
            other => {
                app.push_modal(other);
            }
        }
        return;
    }

    // 2. Normal View (No Modal): Split into layout chunks
    let bottom_height = app.bottom_pane.desired_height(&app.composer);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),             // Header (chunks[0])
            Constraint::Min(5),                // Transcript (chunks[1])
            Constraint::Length(bottom_height), // Bottom pane (chunks[2])
            Constraint::Length(1),             // Footer (chunks[3])
        ])
        .margin(1)
        .split(screen_area);

    let header_area = chunks[0];
    let transcript_area = chunks[1];
    let bottom_area = chunks[2];
    let footer_area = chunks[3];

    // Check Header click
    if header_area.contains(click_pos) {
        if col >= header_area.x + 8 && col <= header_area.x + 18 {
            // Clicked on Collaboration Mode badge ("BUILD" / "PLAN")
            app.toggle_collaboration_mode();
            return;
        }
        if col >= header_area.x + 19 && col <= header_area.x + 36 {
            // Clicked on Active Model name -> open Model Picker
            app.push_modal(ModalState::model_picker(
                app.available_models.clone(),
                &app.active_model,
            ));
            return;
        }
    }

    // Check Footer shortcut click
    if footer_area.contains(click_pos) {
        let center_x = footer_area.x + footer_area.width / 2;
        let start_x = center_x.saturating_sub(40);
        let rel_x = col.saturating_sub(start_x);

        if rel_x < 11 {
            // Palette (Ctrl+P)
            app.open_command_palette();
        } else if rel_x < 21 {
            // Mode (Tab)
            app.toggle_collaboration_mode();
        } else if rel_x < 33 {
            // Models (Ctrl+M)
            app.push_modal(ModalState::model_picker(
                app.available_models.clone(),
                &app.active_model,
            ));
        } else if rel_x < 43 {
            // Diff (Ctrl+D)
            let diff_out = std::process::Command::new("git")
                .args(["diff", "HEAD"])
                .current_dir(&app.cwd)
                .output();
            if let Ok(out) = diff_out {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                let state =
                    m31a_diff::DiffViewerState::from_unified_patch("Git Diff (HEAD)", &text);
                app.push_modal(ModalState::InteractiveDiff(state));
            }
        } else if rel_x < 55 {
            // Resume (Ctrl+R)
            if let Ok(threads) = app.storage.list_threads(20) {
                app.push_modal(ModalState::resume_picker(threads));
            }
        } else if rel_x < 65 {
            // Shell (!)
            app.composer.insert_char('!');
        } else if rel_x < 77 {
            // Cancel (Ctrl+C)
            if let Some(token) = &app.cancel_token {
                token.cancel();
            }
        } else {
            // Help (?)
            app.push_modal(ModalState::help());
        }
        return;
    }

    // Check Bottom pane / Composer click
    if bottom_area.contains(click_pos) {
        // If popup active, check if clicked on candidate
        match &mut app.bottom_pane.mode {
            BottomPaneMode::SlashCommandPopup { candidates, .. } => {
                let popup_height = (candidates.len() as u16 + 2).min(12);
                let popup_y = bottom_area.y.saturating_sub(popup_height);
                if row >= popup_y && row < bottom_area.y {
                    let idx = (row.saturating_sub(popup_y + 1)) as usize;
                    if let Some(cand) = candidates.get(idx) {
                        let name = cand.name.clone();
                        app.composer.set_text(&format!("{} ", name));
                        app.bottom_pane.mode = BottomPaneMode::InputNormal;
                    }
                    return;
                }
            }
            BottomPaneMode::ModelSelectorPopup { .. } => {
                let popup_area =
                    crate::bottom_pane::BottomPaneState::model_selector_popup_area(bottom_area);
                let geometry = crate::model_selector::ModelSelectorGeometry::new(popup_area);
                if let Some(ref mut ms) = app.bottom_pane.model_selector {
                    ms.set_viewport_height(geometry.inner_height());
                    if let Some(rel_row) = geometry.row_to_visible_index(col, row) {
                        let clicked_idx = ms.scroll_offset + rel_row;
                        if clicked_idx < ms.filtered_indices.len() {
                            ms.selected_index = clicked_idx;
                            if let Some(model) = ms.get_selected_model() {
                                let slug = model.slug.clone();
                                match app.switch_model(&slug) {
                                    Ok(_) => {
                                        app.bottom_pane.model_selector = None;
                                        app.bottom_pane.mode = BottomPaneMode::InputNormal;
                                    }
                                    Err(e) => {
                                        app.provider_status_message =
                                            Some(format!("Failed to activate model: {e}"));
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
            _ => {}
        }

        // Plain click inside composer text input area
        let prompt_start_x = bottom_area.x + 3;
        if col >= prompt_start_x {
            let char_offset = (col - prompt_start_x) as usize;
            app.composer.cursor = char_offset.min(app.composer.len());
        }
        return;
    }

    // Check Transcript area click
    if transcript_area.contains(click_pos) {
        if app.current_screen == crate::screens::Screen::Models {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(5)])
                .split(transcript_area);

            // Search bar click
            if chunks[0].contains(click_pos) {
                app.screen_filter_active = true;
                return;
            }

            // Body click
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(chunks[1]);

            // Left catalogue list click
            if body_chunks[0].contains(click_pos) {
                let geometry = crate::model_selector::ModelSelectorGeometry::new(body_chunks[0]);
                if let Some(rel_row) = geometry.row_to_visible_index(col, row)
                    && let Some(ref mut ms) = app.models_screen_state
                {
                    ms.set_viewport_height(geometry.inner_height());
                    let clicked_idx = ms.scroll_offset + rel_row;
                    if clicked_idx < ms.filtered_indices.len() {
                        ms.selected_index = clicked_idx;
                        app.screen_selected_index = clicked_idx;
                    }
                }
                return;
            }

            // Right detail inspector click
            if body_chunks[1].contains(click_pos) {
                return;
            }
        }

        let inner = Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .inner(transcript_area);

        // 1. Click on Unpinned Indicator Pill ("↓ X lines below — End to jump")
        let total_lines = app.transcript_virtualizer.total_line_count(inner.width);
        let offset = app.chat_widget.scroll_controller.offset();
        let viewport_h = inner.height as usize;
        if total_lines > viewport_h && offset + viewport_h < total_lines {
            let pill_y = inner.y + inner.height.saturating_sub(1);
            if row == pill_y && col >= inner.x + inner.width.saturating_sub(35) {
                app.scroll_to_bottom();
                return;
            }
        }

        // 2. Click on Transcript Vertical Scrollbar (rightmost column)
        if col == transcript_area.x + transcript_area.width.saturating_sub(1) {
            if row <= transcript_area.y + 2 {
                app.scroll_up(5);
            } else if row >= transcript_area.y + transcript_area.height.saturating_sub(3) {
                app.scroll_down(5);
            } else {
                let rel_y = row.saturating_sub(transcript_area.y + 1) as f64;
                let track_h = transcript_area.height.saturating_sub(2) as f64;
                let ratio = (rel_y / track_h).clamp(0.0, 1.0);
                let target = (ratio * total_lines as f64) as usize;
                app.chat_widget.scroll_controller.scroll_offset = target;
                app.chat_widget.scroll_controller.clamp_offset();
                app.auto_scroll = app.chat_widget.scroll_controller.is_at_bottom();
                app.scroll_offset = app.chat_widget.scroll_controller.offset();
            }
            return;
        }

        // 3. Click on Transcript Line: Toggle collapsed state for ToolCall / Thinking / ExplorationGroup or start selection
        if inner.contains(click_pos) {
            let line_index = offset + (row.saturating_sub(inner.y)) as usize;
            if app
                .transcript_virtualizer
                .toggle_cell_at_line(line_index, inner.width)
            {
                return;
            }

            // Start drag selection gesture
            app.mouse_selection = Some(MouseSelection::new(col, row));
        }
    }
}

fn handle_left_click_drag(app: &mut App, col: u16, row: u16, _screen_area: Rect) {
    if let Some(sel) = &mut app.mouse_selection {
        sel.current = (col, row);
        if sel.current != sel.start {
            sel.active = true;
        }
    }
}

fn handle_left_click_up(app: &mut App, col: u16, row: u16, screen_area: Rect) {
    if let Some(mut sel) = app.mouse_selection.take() {
        sel.current = (col, row);
        if sel.active && sel.start != sel.current {
            // Extract selected transcript lines and optionally copy to clipboard
            if let Some(text) = extract_selected_text(app, sel, screen_area)
                && !text.trim().is_empty()
                && app.config.tui.copy_on_select
            {
                crate::clipboard::copy_to_clipboard(&text);
                let preview = if text.chars().count() > 32 {
                    let truncated: String = text.chars().take(30).collect();
                    format!("{}…", truncated)
                } else {
                    text.clone()
                };
                app.transcript.push(TranscriptItem::notice(
                    format!("Copied to clipboard: \"{}\"", preview.replace('\n', " ")),
                    NoticeLevel::Success,
                ));
                app.sync_virtualizer();
            }
        }
    }
}

/// Extracts text within the selected rectangular terminal region.
fn extract_selected_text(app: &App, sel: MouseSelection, screen_area: Rect) -> Option<String> {
    let bottom_height = app.bottom_pane.desired_height(&app.composer);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(bottom_height),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(screen_area);

    let inner = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .inner(chunks[1]);

    let min_row = sel.start.1.min(sel.current.1);
    let max_row = sel.start.1.max(sel.current.1);

    if max_row < inner.y || min_row >= inner.y + inner.height {
        return None;
    }

    let clamped_min_row = min_row.max(inner.y);
    let clamped_max_row = max_row.min(inner.y + inner.height.saturating_sub(1));

    let offset = app.chat_widget.scroll_controller.offset();
    let mut extracted_lines = Vec::new();

    for r in clamped_min_row..=clamped_max_row {
        let line_idx = offset + (r - inner.y) as usize;
        if let Some(line_str) = app
            .transcript_virtualizer
            .get_line_as_string(line_idx, inner.width)
        {
            extracted_lines.push(line_str);
        }
    }

    if extracted_lines.is_empty() {
        None
    } else {
        Some(extracted_lines.join("\n"))
    }
}

pub fn settings_items_count(tab: usize) -> usize {
    match tab {
        0 => 4, // General: Model, Reasoning Effort, Dynamic Routing, Web Search
        1 => 4, // Security: Sandbox, Approval, Login Shell, Hooks
        2 => 7, // TUI: Theme, Animations, Vim Mode, Mouse Support, Scroll Acceleration, Copy on Select, Scroll Speed
        3 => 6, // Features: Memories, Goals, Max Depth, Max Threads, Code Mode, Guardian
        _ => 1,
    }
}

pub fn toggle_or_cycle_setting(app: &mut App, active_tab: usize, selected_index: usize) {
    match active_tab {
        0 => {
            // General
            match selected_index {
                0 => {
                    if !app.available_models.is_empty() {
                        let cur_idx = app
                            .available_models
                            .iter()
                            .position(|m| m.slug == app.active_model)
                            .unwrap_or(0);
                        let next_idx = (cur_idx + 1) % app.available_models.len();
                        let next_slug = app.available_models[next_idx].slug.clone();
                        if let Err(e) = app.switch_model(&next_slug) {
                            app.provider_status_message = Some(format!("Model switch failed: {e}"));
                        }
                    }
                }
                1 => {
                    let next = match app.config.reasoning_effort.as_deref() {
                        Some("none") | None => Some("low".to_string()),
                        Some("low") => Some("medium".to_string()),
                        Some("medium") => Some("high".to_string()),
                        Some("high") => None,
                        _ => Some("low".to_string()),
                    };
                    app.config.reasoning_effort = next.clone();
                    app.nim_client = app.nim_client.clone().with_reasoning_effort(next.clone());
                    app.bottom_pane.reasoning_effort = next;
                }
                2 => {
                    app.config.routing_enabled = !app.config.routing_enabled;
                }
                3 => {
                    app.config.web_search = match app.config.web_search.as_str() {
                        "live" => "duckduckgo".to_string(),
                        "duckduckgo" => "disabled".to_string(),
                        _ => "live".to_string(),
                    };
                }
                _ => {}
            }
        }
        1 => {
            // Security
            match selected_index {
                0 => {
                    app.config.sandbox = match app.config.sandbox {
                        m31a_core::SandboxPolicy::ReadOnly => {
                            m31a_core::SandboxPolicy::WorkspaceWrite
                        }
                        m31a_core::SandboxPolicy::WorkspaceWrite => {
                            m31a_core::SandboxPolicy::DangerFullAccess
                        }
                        m31a_core::SandboxPolicy::DangerFullAccess => {
                            m31a_core::SandboxPolicy::ReadOnly
                        }
                    };
                    app.refresh_system_prompt();
                }
                1 => {
                    app.config.approval = match app.config.approval {
                        m31a_core::ApprovalPolicy::OnRequest => {
                            m31a_core::ApprovalPolicy::ApproveForMe
                        }
                        m31a_core::ApprovalPolicy::ApproveForMe => m31a_core::ApprovalPolicy::Never,
                        m31a_core::ApprovalPolicy::Never => m31a_core::ApprovalPolicy::OnRequest,
                    };
                }
                2 => {
                    app.config.allow_login_shell = !app.config.allow_login_shell;
                }
                3 => {
                    app.config.hooks.enabled = !app.config.hooks.enabled;
                }
                _ => {}
            }
        }
        2 => {
            // TUI & Appearance
            match selected_index {
                0 => {
                    let current = app
                        .config
                        .tui
                        .theme
                        .as_deref()
                        .unwrap_or("dark")
                        .to_lowercase();
                    let (next_variant, name) = match current.as_str() {
                        "dark" => (crate::theme::ThemeVariant::Light, "light"),
                        "light" => (crate::theme::ThemeVariant::HighContrast, "contrast"),
                        _ => (crate::theme::ThemeVariant::Dark, "dark"),
                    };
                    crate::theme::init_theme(next_variant);
                    app.config.tui.theme = Some(name.to_string());
                }
                1 => {
                    app.config.tui.animations = !app.config.tui.animations;
                }
                2 => {
                    app.config.tui.vim_mode_default = !app.config.tui.vim_mode_default;
                }
                3 => {
                    app.config.tui.mouse_enabled = !app.config.tui.mouse_enabled;
                    // Live toggle terminal mouse capture
                    let mut stdout = std::io::stdout();
                    if app.config.tui.mouse_enabled {
                        let _ = crossterm::execute!(stdout, crossterm::event::EnableMouseCapture);
                    } else {
                        let _ = crossterm::execute!(stdout, crossterm::event::DisableMouseCapture);
                    }
                }
                4 => {
                    app.config.tui.scroll_acceleration = !app.config.tui.scroll_acceleration;
                }
                5 => {
                    app.config.tui.copy_on_select = !app.config.tui.copy_on_select;
                }
                6 => {
                    app.config.tui.mouse_scroll_speed =
                        match app.config.tui.mouse_scroll_speed as u32 {
                            1 => 2.0,
                            2 => 3.0,
                            3 => 0.5,
                            _ => 1.0,
                        };
                }
                _ => {}
            }
        }
        3 => {
            // Features & Agent
            match selected_index {
                0 => {
                    app.config.memories.enabled = !app.config.memories.enabled;
                }
                1 => {
                    app.config.goals.enabled = !app.config.goals.enabled;
                }
                2 => {
                    app.config.agents.max_depth = match app.config.agents.max_depth {
                        1 => 2,
                        2 => 3,
                        3 => 5,
                        _ => 1,
                    };
                }
                3 => {
                    app.config.agents.max_threads = match app.config.agents.max_threads {
                        4 => 8,
                        8 => 16,
                        16 => 32,
                        _ => 4,
                    };
                }
                4 => {
                    let cur = app
                        .config
                        .features
                        .get("code_mode")
                        .copied()
                        .unwrap_or(true);
                    app.config.features.insert("code_mode".to_string(), !cur);
                }
                5 => {
                    let cur = app
                        .config
                        .features
                        .get("guardian_v2")
                        .copied()
                        .unwrap_or(true);
                    app.config.features.insert("guardian_v2".to_string(), !cur);
                }
                _ => {}
            }
        }
        _ => {}
    }

    if let Err(e) = app.config.save() {
        tracing::warn!("Failed to persist config after setting change: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use m31a_core::config::Config;
    use ratatui::layout::Rect;

    fn make_test_app() -> App {
        let config = Config {
            api_key: Some("test_key".to_string()),
            ..Default::default()
        };
        App::new(config).unwrap()
    }

    #[test]
    fn test_mouse_disabled_ignores_events() {
        let mut app = make_test_app();
        app.config.tui.mouse_enabled = false;
        app.push_modal(ModalState::HelpModal { scroll: 0 });

        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, event, Rect::new(0, 0, 100, 40));

        // Modal should NOT be dismissed because mouse is disabled
        assert!(app.has_active_modal());
    }

    #[test]
    fn test_middle_click_closes_modal() {
        let mut app = make_test_app();
        app.push_modal(ModalState::HelpModal { scroll: 0 });

        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Middle),
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, event, Rect::new(0, 0, 100, 40));

        assert!(!app.has_active_modal());
    }

    #[test]
    fn test_backdrop_dismissal() {
        let mut app = make_test_app();
        app.push_modal(ModalState::HelpModal { scroll: 0 });

        // Screen is 100x40; HelpModal centered_rect(80, 80) occupies x: 10..90, y: 4..36.
        // Clicking at (0, 0) is well outside the popup.
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, event, Rect::new(0, 0, 100, 40));

        assert!(!app.has_active_modal());
    }

    #[test]
    fn test_mouse_scroll_diff_viewer() {
        let mut app = make_test_app();
        app.push_modal(ModalState::DiffViewer {
            diff_text: vec![
                "line1".into(),
                "line2".into(),
                "line3".into(),
                "line4".into(),
                "line5".into(),
                "line6".into(),
                "line7".into(),
                "line8".into(),
                "line9".into(),
                "line10".into(),
            ],
            scroll: 0,
        });

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, scroll_down, Rect::new(0, 0, 100, 40));

        if let Some(ModalState::DiffViewer { scroll, .. }) = app.current_modal() {
            assert!(
                *scroll > 0,
                "DiffViewer scroll should increase on scroll down"
            );
        } else {
            panic!("Expected DiffViewer modal to remain active");
        }

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 50,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, scroll_up, Rect::new(0, 0, 100, 40));

        if let Some(ModalState::DiffViewer { scroll, .. }) = app.current_modal() {
            assert_eq!(
                *scroll, 0,
                "DiffViewer scroll should decrease to 0 on scroll up"
            );
        } else {
            panic!("Expected DiffViewer modal to remain active");
        }
    }

    #[test]
    fn test_header_badge_click_mode_toggle() {
        use m31a_core::prompts::CollaborationMode;
        let mut app = make_test_app();
        assert_eq!(app.collaboration_mode, CollaborationMode::Default);

        // Header rect is row 1..4 with margin(1); badge is around col 9..19, row 2
        let click_mode = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, click_mode, Rect::new(0, 0, 100, 40));
        assert_eq!(app.collaboration_mode, CollaborationMode::Plan);

        // Click again to toggle back
        handle_mouse_event(&mut app, click_mode, Rect::new(0, 0, 100, 40));
        assert_eq!(app.collaboration_mode, CollaborationMode::Default);
    }

    #[test]
    fn test_mouse_selection_lifecycle() {
        let mut app = make_test_app();
        let screen = Rect::new(0, 0, 100, 40);

        // Down inside transcript area (row 10, col 20)
        let down_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 20,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, down_event, screen);
        assert!(app.mouse_selection.is_some());
        let sel = app.mouse_selection.unwrap();
        assert_eq!(sel.start, (20, 10));
        assert_eq!(sel.current, (20, 10));

        // Drag to (35, 12)
        let drag_event = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 35,
            row: 12,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, drag_event, screen);
        let sel = app.mouse_selection.unwrap();
        assert_eq!(sel.start, (20, 10));
        assert_eq!(sel.current, (35, 12));
        assert!(sel.active);

        // Release at (35, 12)
        let up_event = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 35,
            row: 12,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(&mut app, up_event, screen);
        assert!(app.mouse_selection.is_none());
    }

    #[test]
    fn test_settings_toggle_via_mouse() {
        let mut app = make_test_app();

        // Initial settings check
        assert!(app.config.tui.mouse_enabled);
        assert!(app.config.tui.scroll_acceleration);
        assert!(app.config.tui.copy_on_select);
        assert_eq!(app.config.tui.mouse_scroll_speed, 1.0);

        // Tab 2 (TUI), Index 3: Terminal Mouse Support
        toggle_or_cycle_setting(&mut app, 2, 3);
        assert!(!app.config.tui.mouse_enabled);

        // Tab 2 (TUI), Index 4: Scroll Acceleration
        toggle_or_cycle_setting(&mut app, 2, 4);
        assert!(!app.config.tui.scroll_acceleration);

        // Tab 2 (TUI), Index 5: Copy on Select
        toggle_or_cycle_setting(&mut app, 2, 5);
        assert!(!app.config.tui.copy_on_select);

        // Tab 2 (TUI), Index 6: Mouse Scroll Speed
        toggle_or_cycle_setting(&mut app, 2, 6);
        assert_eq!(app.config.tui.mouse_scroll_speed, 2.0);
    }
}
