//! Terminal event listener, raw mode manager, and keybinding dispatcher.
//!
//! Initializes raw terminal mode, listens for async Crossterm keyboard/mouse events, and restores terminal on exit.

use anyhow::Result;
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::stdout;
use std::time::Duration;

use crate::app::{App, AppStatus, TuiEvent};
use crate::modals::ModalState;
use crate::renderer;

/// RAII guard guaranteeing terminal mode restoration on normal exit, error return, or panic unwind.
pub struct TerminalSessionGuard {
    pub keyboard_enhanced: bool,
    pub mouse_captured: bool,
    pub active: bool,
}

impl TerminalSessionGuard {
    pub fn enter(mouse_enabled: bool) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();

        let keyboard_enhanced = matches!(supports_keyboard_enhancement(), Ok(true));
        if keyboard_enhanced {
            let _ = execute!(
                stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            );
        }

        if mouse_enabled {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste,
                crossterm::event::EnableFocusChange
            )?;
        } else {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableBracketedPaste,
                crossterm::event::EnableFocusChange
            )?;
        }

        Ok(Self {
            keyboard_enhanced,
            mouse_captured: mouse_enabled,
            active: true,
        })
    }

    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut stdout = stdout();
        if self.keyboard_enhanced {
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(stdout, crossterm::event::DisableFocusChange);
        let _ = disable_raw_mode();
        let _ = execute!(
            stdout,
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste,
            crossterm::cursor::Show
        );
        Ok(())
    }
}

impl Drop for TerminalSessionGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub async fn run_tui_app(mut app: App, initial_prompt: Option<String>) -> Result<()> {
    let mut guard = TerminalSessionGuard::enter(app.config.tui.mouse_enabled)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    if let Some(prompt) = initial_prompt {
        app.submit_prompt(prompt);
    }

    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(33)); // ~30 fps

    loop {
        app.poll_events();
        terminal.draw(|f| renderer::render(f, &mut app))?;

        if app.should_quit {
            break;
        }

        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
            }
            maybe_event = event_stream.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::FocusGained => {
                            app.is_focused = true;
                        }
                        Event::FocusLost => {
                            app.is_focused = false;
                        }
                        Event::Key(key) => {
                            handle_key_event(&mut app, key);
                        }
                        Event::Paste(pasted_str) => {
                            handle_paste_event(&mut app, pasted_str);
                        }
                        Event::Mouse(mouse) => {
                            let area = terminal.get_frame().area();
                            crate::mouse::handle_mouse_event(&mut app, mouse, area);
                        }
                        Event::Resize(_, _) => {
                            // Ratatui handles resize on next draw
                        }
                    }
                }
            }
        }
    }

    guard.restore()?;
    Ok(())
}

fn handle_paste_event(app: &mut App, pasted_str: String) {
    if app.has_active_modal() {
        return;
    }

    if let Some(pill) = app.paste_detector.flush_if_large_paste(&pasted_str) {
        let snippet = format!("\n```\n{}\n```\n", pill.full_content);
        app.composer.insert_str(&snippet);
        app.transcript
            .push(crate::transcript::TranscriptItem::notice(
                format!(
                    "Attached pasted snippet: {} ({} lines)",
                    pill.title, pill.line_count
                ),
                crate::transcript::NoticeLevel::Info,
            ));
    } else {
        app.bottom_pane.on_paste(&pasted_str, &mut app.composer);
    }
}

pub fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent) {
    if let Some(modal) = app.pop_modal() {
        handle_modal_key_event(app, modal, key);
        return;
    }

    if handle_global_shortcut_event(app, key) {
        return;
    }

    if app.current_screen != crate::screens::Screen::Chat {
        handle_screen_key_event(app, key);
        return;
    }

    handle_composer_key_event(app, key);
}

pub fn handle_modal_key_event(app: &mut App, modal: ModalState, key: crossterm::event::KeyEvent) {
    match modal {
        ModalState::AnalyticsDashboard => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::AnalyticsDashboard);
            }
        },
        ModalState::ExperimentalFeatures {
            mut items,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if selected_index + 1 < items.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                });
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some((key, _name, _desc, enabled)) = items.get_mut(selected_index) {
                    *enabled = !*enabled;
                    app.config
                        .features
                        .insert(key.as_str().to_string(), *enabled);
                    if let Err(e) = app.config.save() {
                        tracing::warn!("Failed to persist config after feature toggle: {e}");
                    }
                }
                app.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                });
            }
            _ => {
                app.push_modal(ModalState::ExperimentalFeatures {
                    items,
                    selected_index,
                });
            }
        },
        ModalState::InteractiveDiff(mut s) => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                s.scroll_up(1);
                app.push_modal(ModalState::InteractiveDiff(s));
            }
            KeyCode::Down => {
                s.scroll_down(1);
                app.push_modal(ModalState::InteractiveDiff(s));
            }
            KeyCode::PageUp => {
                s.scroll_page_up();
                app.push_modal(ModalState::InteractiveDiff(s));
            }
            KeyCode::PageDown => {
                s.scroll_page_down();
                app.push_modal(ModalState::InteractiveDiff(s));
            }
            _ => {
                app.push_modal(ModalState::InteractiveDiff(s));
            }
        },
        ModalState::QuestionPrompt(mut q) => match key.code {
            KeyCode::Esc => {
                if let Some(tx) = app.question_tx.take() {
                    let _ = tx.send(Vec::new());
                }
                // closed (already popped)
            }
            KeyCode::Up => {
                q.move_up();
                app.push_modal(ModalState::QuestionPrompt(q));
            }
            KeyCode::Down => {
                q.move_down();
                app.push_modal(ModalState::QuestionPrompt(q));
            }
            KeyCode::Char(' ') => {
                q.toggle_selection();
                app.push_modal(ModalState::QuestionPrompt(q));
            }
            KeyCode::Enter => {
                let answers = q.submit();
                if let Some(tx) = app.question_tx.take() {
                    let _ = tx.send(answers);
                }
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::QuestionPrompt(q));
            }
        },
        ModalState::ModelPicker { mut state } => match key.code {
            KeyCode::Esc | KeyCode::F(2) => {
                // closed (already popped)
            }
            KeyCode::Char('m') | KeyCode::Char('M')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                // closed (already popped)
            }
            KeyCode::Up => {
                state.select_prev();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Down => {
                state.select_next();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::PageUp => {
                state.select_page_up();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::PageDown => {
                state.select_page_down();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Home => {
                state.select_home();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::End => {
                state.select_end();
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Char('/') => {
                state.set_search_query(String::new());
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Backspace => {
                let mut q = state.search_query.clone();
                q.pop();
                state.set_search_query(q);
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                let mut q = state.search_query.clone();
                q.push(c);
                state.set_search_query(q);
                app.push_modal(ModalState::ModelPicker { state });
            }
            KeyCode::Enter => {
                if let Some(m) = state.get_selected_model() {
                    let chosen = m.slug.clone();
                    match app.switch_model(&chosen) {
                        Ok(_) => {
                            // Successful activation closes the modal
                        }
                        Err(e) => {
                            // Failed activation preserves modal and reports actionable error
                            app.provider_status_message =
                                Some(format!("Failed to activate model: {e}"));
                            app.push_modal(ModalState::ModelPicker { state });
                        }
                    }
                } else {
                    app.push_modal(ModalState::ModelPicker { state });
                }
            }
            _ => {
                app.push_modal(ModalState::ModelPicker { state });
            }
        },
        ModalState::ApprovalDialog {
            approval,
            mut selected_option,
        } => {
            let actions = approval.action_kinds();
            let max_index = actions.len().saturating_sub(1);
            selected_option = selected_option.min(max_index);
            let requested = match key.code {
                KeyCode::Left | KeyCode::Up | KeyCode::Char('k') => {
                    selected_option = selected_option.saturating_sub(1);
                    None
                }
                KeyCode::Right | KeyCode::Down | KeyCode::Char('j') => {
                    if selected_option < max_index {
                        selected_option += 1;
                    }
                    None
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    Some(crate::modals::ApprovalActionKind::AllowOnce)
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    Some(crate::modals::ApprovalActionKind::AllowRule)
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    Some(crate::modals::ApprovalActionKind::AllowFile)
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    Some(crate::modals::ApprovalActionKind::ViewDiff)
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    Some(crate::modals::ApprovalActionKind::DeclineWithFeedback)
                }
                KeyCode::Esc => Some(crate::modals::ApprovalActionKind::Cancel),
                KeyCode::Enter => actions.get(selected_option).copied(),
                _ => None,
            };

            if let Some(action) = requested
                && actions.contains(&action)
            {
                app.resolve_approval_action(approval.clone(), action);
                return;
            }
            app.push_modal(ModalState::ApprovalDialog {
                approval,
                selected_option,
            });
        }
        ModalState::DeclineFeedbackInput {
            approval,
            mut feedback_buffer,
        } => {
            match key.code {
                KeyCode::Esc => {
                    // Return to the approval decision without denying anything.
                    app.push_modal(ModalState::ApprovalDialog {
                        approval,
                        selected_option: 0,
                    });
                }
                KeyCode::Enter => {
                    let reason = if feedback_buffer.trim().is_empty() {
                        "Operation denied by user.".to_string()
                    } else {
                        feedback_buffer
                    };
                    app.send_approval_decision(m31a_elicitation::ApprovalDecision::Deny { reason });
                    // closed (already popped)
                }
                KeyCode::Backspace => {
                    feedback_buffer.pop();
                    app.push_modal(ModalState::DeclineFeedbackInput {
                        approval,
                        feedback_buffer,
                    });
                }
                KeyCode::Char(c) => {
                    feedback_buffer.push(c);
                    app.push_modal(ModalState::DeclineFeedbackInput {
                        approval,
                        feedback_buffer,
                    });
                }
                _ => {
                    app.push_modal(ModalState::DeclineFeedbackInput {
                        approval,
                        feedback_buffer,
                    });
                }
            }
        }
        ModalState::QuickDiffPreview {
            diff_text,
            return_to_approval,
            mut scroll,
        } => {
            match key.code {
                KeyCode::Esc => {
                    // Drop cleanly right back into the approval dialog!
                    app.push_modal(ModalState::ApprovalDialog {
                        approval: return_to_approval,
                        selected_option: 0,
                    });
                }
                KeyCode::Up => {
                    scroll = scroll.saturating_sub(1);
                    app.push_modal(ModalState::QuickDiffPreview {
                        diff_text,
                        return_to_approval,
                        scroll,
                    });
                }
                KeyCode::Down => {
                    if scroll + 5 < diff_text.len() {
                        scroll += 1;
                    }
                    app.push_modal(ModalState::QuickDiffPreview {
                        diff_text,
                        return_to_approval,
                        scroll,
                    });
                }
                KeyCode::PageUp => {
                    scroll = scroll.saturating_sub(10);
                    app.push_modal(ModalState::QuickDiffPreview {
                        diff_text,
                        return_to_approval,
                        scroll,
                    });
                }
                KeyCode::PageDown => {
                    scroll = (scroll + 10).min(diff_text.len().saturating_sub(1));
                    app.push_modal(ModalState::QuickDiffPreview {
                        diff_text,
                        return_to_approval,
                        scroll,
                    });
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.resolve_approval_action(
                        return_to_approval,
                        crate::modals::ApprovalActionKind::AllowOnce,
                    );
                    // closed (already popped)
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    app.push_modal(ModalState::DeclineFeedbackInput {
                        approval: return_to_approval,
                        feedback_buffer: String::new(),
                    });
                }
                _ => {
                    app.push_modal(ModalState::QuickDiffPreview {
                        diff_text,
                        return_to_approval,
                        scroll,
                    });
                }
            }
        }
        ModalState::ResumePicker(mut picker) => match key.code {
            KeyCode::Esc => {
                // closed (already popped)
            }
            KeyCode::Up => {
                picker.move_up();
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Down => {
                picker.move_down();
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Backspace => {
                picker.pop_char();
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Char('n') if picker.search_query.is_empty() => {
                let _ = app.create_new_session(None);
                // closed (already popped)
            }
            KeyCode::Char('f') if picker.search_query.is_empty() => {
                if let Some(t) = picker.selected_thread() {
                    let parent_id = t.id.clone();
                    let title = format!("{} (Fork)", t.title);
                    if let Ok(forked) = app.storage.fork_thread(&parent_id, &title) {
                        app.session_id = forked.id.clone();
                        if let Err(e) = app.switch_model(&forked.model) {
                            app.provider_status_message =
                                Some(format!("Model activation failed: {e}"));
                        }
                        app.transcript
                            .push(crate::transcript::TranscriptItem::notice(
                                format!("Forked session into '{}' ({})", forked.title, forked.id),
                                crate::transcript::NoticeLevel::Success,
                            ));
                    }
                }
                // closed (already popped)
            }
            KeyCode::Char('a') if picker.search_query.is_empty() => {
                if let Some(t) = picker.selected_thread() {
                    let id = t.id.clone();
                    if t.is_archived {
                        let _ = app.storage.unarchive_thread(&id);
                    } else {
                        let _ = app.storage.archive_thread(&id);
                    }
                    if let Ok(threads) = app.storage.list_threads(50) {
                        picker = crate::resume_picker::ResumePickerState::new(threads);
                    }
                }
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Char('d') if picker.search_query.is_empty() => {
                if let Some(t) = picker.selected_thread() {
                    let id = t.id.clone();
                    let _ = app.storage.delete_thread(&id);
                    if let Ok(threads) = app.storage.list_threads(50) {
                        picker = crate::resume_picker::ResumePickerState::new(threads);
                    }
                }
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Char(c) => {
                picker.push_char(c);
                app.push_modal(ModalState::ResumePicker(picker));
            }
            KeyCode::Enter => {
                if let Some(t) = picker.selected_thread() {
                    let tid = t.id.clone();
                    if let Err(e) = app.resume_session(&tid) {
                        app.provider_status_message =
                            Some(format!("Failed to resume session: {e}"));
                        app.push_modal(ModalState::ResumePicker(picker));
                    }
                }
                // closed (if successfully resumed)
            }
            _ => {
                app.push_modal(ModalState::ResumePicker(picker));
            }
        },
        ModalState::DiffViewer {
            diff_text,
            mut scroll,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                app.push_modal(ModalState::DiffViewer { diff_text, scroll });
            }
            KeyCode::Down => {
                scroll += 1;
                app.push_modal(ModalState::DiffViewer { diff_text, scroll });
            }
            KeyCode::PageUp => {
                scroll = scroll.saturating_sub(10);
                app.push_modal(ModalState::DiffViewer { diff_text, scroll });
            }
            KeyCode::PageDown => {
                scroll += 10;
                app.push_modal(ModalState::DiffViewer { diff_text, scroll });
            }
            _ => {
                app.push_modal(ModalState::DiffViewer { diff_text, scroll });
            }
        },
        ModalState::SymbolOutlineModal {
            file_path,
            symbols,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::SymbolOutlineModal {
                    file_path,
                    symbols,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if selected_index + 1 < symbols.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::SymbolOutlineModal {
                    file_path,
                    symbols,
                    selected_index,
                });
            }
            KeyCode::Enter => {
                if let Some(sym) = symbols.get(selected_index) {
                    let note = format!(
                        "Selected symbol '{}' ({:?}) at {}:{}",
                        sym.name, sym.kind, file_path, sym.line
                    );
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            note,
                            crate::transcript::NoticeLevel::Info,
                        ));
                }
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::SymbolOutlineModal {
                    file_path,
                    symbols,
                    selected_index,
                });
            }
        },
        ModalState::CriticalPathModal {
            durations,
            completed_intervals,
            mut scroll,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                // closed (already popped)
            }
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                app.push_modal(ModalState::CriticalPathModal {
                    durations,
                    completed_intervals,
                    scroll,
                });
            }
            KeyCode::Down => {
                if scroll + 1 < completed_intervals.len() {
                    scroll += 1;
                }
                app.push_modal(ModalState::CriticalPathModal {
                    durations,
                    completed_intervals,
                    scroll,
                });
            }
            _ => {
                app.push_modal(ModalState::CriticalPathModal {
                    durations,
                    completed_intervals,
                    scroll,
                });
            }
        },
        ModalState::ActiveLocksModal {
            locks,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::ActiveLocksModal {
                    locks,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !locks.is_empty() && selected_index + 1 < locks.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::ActiveLocksModal {
                    locks,
                    selected_index,
                });
            }
            _ => {
                app.push_modal(ModalState::ActiveLocksModal {
                    locks,
                    selected_index,
                });
            }
        },
        ModalState::CheckpointsModal {
            checkpoints,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::CheckpointsModal {
                    checkpoints,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !checkpoints.is_empty() && selected_index + 1 < checkpoints.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::CheckpointsModal {
                    checkpoints,
                    selected_index,
                });
            }
            KeyCode::Enter if !checkpoints.is_empty() && selected_index < checkpoints.len() => {
                let target_cp = checkpoints[selected_index].clone();
                app.push_modal(ModalState::revert_confirm(target_cp));
            }
            _ => {
                app.push_modal(ModalState::CheckpointsModal {
                    checkpoints,
                    selected_index,
                });
            }
        },
        ModalState::RevertConfirmModal {
            checkpoint,
            mut selected_option,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') | KeyCode::Char('N') => {
                let cps = app
                    .storage
                    .list_checkpoints(&app.session_id)
                    .unwrap_or_default();
                app.push_modal(ModalState::checkpoints_modal(cps));
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                selected_option = if selected_option == 0 { 1 } else { 0 };
                app.push_modal(ModalState::RevertConfirmModal {
                    checkpoint,
                    selected_option,
                });
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let _ = app.restore_checkpoint_action(&checkpoint);
                // closed (already popped)
            }
            KeyCode::Enter => {
                if selected_option == 0 {
                    let _ = app.restore_checkpoint_action(&checkpoint);
                    // closed (already popped)
                } else {
                    let cps = app
                        .storage
                        .list_checkpoints(&app.session_id)
                        .unwrap_or_default();
                    app.push_modal(ModalState::checkpoints_modal(cps));
                }
            }
            _ => {
                app.push_modal(ModalState::RevertConfirmModal {
                    checkpoint,
                    selected_option,
                });
            }
        },
        ModalState::ProcessesModal {
            processes,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::ProcessesModal {
                    processes,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !processes.is_empty() && selected_index + 1 < processes.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::ProcessesModal {
                    processes,
                    selected_index,
                });
            }
            KeyCode::Enter | KeyCode::Char('v') => {
                if !processes.is_empty() && selected_index < processes.len() {
                    let p = &processes[selected_index];
                    let proc_id = p.id.clone();
                    let label = p.label.clone();
                    let supervisor = app.proc_supervisor.clone();
                    let tx = app.event_tx.clone();
                    let pid_clone = proc_id.clone();
                    let label_clone = label.clone();
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        handle.spawn(async move {
                            let lines = supervisor
                                .tail_output(&pid_clone, 200)
                                .await
                                .unwrap_or_default();
                            let _ = tx
                                .send(TuiEvent::ProcessOutputReady {
                                    id: pid_clone,
                                    label: label_clone,
                                    lines,
                                })
                                .await;
                        });
                    }
                    app.push_modal(ModalState::ProcessesModal {
                        processes,
                        selected_index,
                    });
                    app.push_modal(ModalState::process_viewer(proc_id, label, Vec::new()));
                } else {
                    app.push_modal(ModalState::ProcessesModal {
                        processes,
                        selected_index,
                    });
                }
            }
            KeyCode::Char('k') if !processes.is_empty() && selected_index < processes.len() => {
                let p = &processes[selected_index];
                let proc_id = p.id.clone();
                let supervisor = app.proc_supervisor.clone();
                let tx = app.event_tx.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = supervisor.stop(&proc_id, false).await;
                        let updated = supervisor.list().await;
                        let _ = tx.send(TuiEvent::ProcessListUpdated(updated)).await;
                    });
                }
                app.push_modal(ModalState::ProcessesModal {
                    processes,
                    selected_index,
                });
            }
            _ => {
                app.push_modal(ModalState::ProcessesModal {
                    processes,
                    selected_index,
                });
            }
        },
        ModalState::ProcessViewer {
            process_id,
            label,
            lines,
            mut scroll,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Closed; parent modal on stack (ProcessesModal) restored
            }
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                app.push_modal(ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                });
            }
            KeyCode::Down => {
                if scroll + 1 < lines.len() {
                    scroll += 1;
                }
                app.push_modal(ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                });
            }
            KeyCode::PageUp => {
                scroll = scroll.saturating_sub(10);
                app.push_modal(ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                });
            }
            KeyCode::PageDown => {
                if scroll + 10 < lines.len() {
                    scroll += 10;
                }
                app.push_modal(ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                });
            }
            KeyCode::Char('k') => {
                let supervisor = app.proc_supervisor.clone();
                let tx = app.event_tx.clone();
                let pid = process_id.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = supervisor.stop(&pid, false).await;
                        let updated = supervisor.list().await;
                        let _ = tx.send(TuiEvent::ProcessListUpdated(updated)).await;
                    });
                }
                // Process stopped; viewer closed, parent restored
            }
            _ => {
                app.push_modal(ModalState::ProcessViewer {
                    process_id,
                    label,
                    lines,
                    scroll,
                });
            }
        },
        ModalState::HelpModal { mut scroll } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                // closed (already popped)
            }
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                app.push_modal(ModalState::HelpModal { scroll });
            }
            KeyCode::Down => {
                scroll += 1;
                app.push_modal(ModalState::HelpModal { scroll });
            }
            _ => {
                app.push_modal(ModalState::HelpModal { scroll });
            }
        },
        ModalState::CommandPalette {
            mut query,
            mut selected_index,
            items,
            mut filtered_indices,
        } => match key.code {
            KeyCode::Esc => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                });
            }
            KeyCode::Down => {
                if !filtered_indices.is_empty() && selected_index + 1 < filtered_indices.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                });
            }
            KeyCode::Backspace => {
                query.pop();
                filtered_indices = filter_palette_items(&query, &items);
                selected_index = 0;
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                });
            }
            KeyCode::Char(c) => {
                query.push(c);
                filtered_indices = filter_palette_items(&query, &items);
                selected_index = 0;
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                });
            }
            KeyCode::Enter => {
                if let Some(&item_idx) = filtered_indices.get(selected_index)
                    && let Some(item) = items.get(item_idx)
                {
                    let action_id = item.id.clone();
                    // closed (already popped)
                    execute_palette_action(app, &action_id);
                }
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::CommandPalette {
                    query,
                    selected_index,
                    items,
                    filtered_indices,
                });
            }
        },
        ModalState::SettingsModal {
            mut active_tab,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                match app.config.save() {
                    Ok(_) => {
                        let path = m31a_core::Config::config_path();
                        app.transcript
                            .push(crate::transcript::TranscriptItem::notice(
                                format!("Settings saved to {}", path.display()),
                                crate::transcript::NoticeLevel::Success,
                            ));
                    }
                    Err(e) => {
                        app.transcript
                            .push(crate::transcript::TranscriptItem::notice(
                                format!("Failed to save settings: {e}"),
                                crate::transcript::NoticeLevel::Warning,
                            ));
                    }
                }
                app.sync_virtualizer();
            }
            KeyCode::Tab | KeyCode::Right => {
                active_tab = (active_tab + 1) % 4;
                let max_items = settings_items_count(active_tab);
                selected_index = selected_index.min(max_items.saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::BackTab | KeyCode::Left => {
                active_tab = if active_tab == 0 { 3 } else { active_tab - 1 };
                let max_items = settings_items_count(active_tab);
                selected_index = selected_index.min(max_items.saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Char('1') => {
                active_tab = 0;
                selected_index = selected_index.min(settings_items_count(0).saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Char('2') => {
                active_tab = 1;
                selected_index = selected_index.min(settings_items_count(1).saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Char('3') => {
                active_tab = 2;
                selected_index = selected_index.min(settings_items_count(2).saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Char('4') => {
                active_tab = 3;
                selected_index = selected_index.min(settings_items_count(3).saturating_sub(1));
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Down => {
                let max_items = settings_items_count(active_tab);
                if selected_index + 1 < max_items {
                    selected_index += 1;
                }
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                toggle_or_cycle_setting(app, active_tab, selected_index);
                if let Err(e) = app.config.save() {
                    tracing::warn!("Failed to persist settings: {e}");
                }
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
            _ => {
                app.push_modal(ModalState::SettingsModal {
                    active_tab,
                    selected_index,
                });
            }
        },
        ModalState::WorktreeSelector {
            worktrees,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::WorktreeSelector {
                    worktrees,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !worktrees.is_empty() && selected_index + 1 < worktrees.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::WorktreeSelector {
                    worktrees,
                    selected_index,
                });
            }
            KeyCode::Enter => {
                if let Some(target) = worktrees.get(selected_index) {
                    let _ = app.switch_workspace(target.path.clone());
                }
                app.pop_modal();
            }
            KeyCode::Char('n') | KeyCode::Char('c') => {
                if let Ok(mgr) = m31a_core::worktree::WorktreeManager::new() {
                    match mgr.create_worktree(&app.cwd, None) {
                        Ok(new_wt) => {
                            let _ = app.switch_workspace(new_wt.path.clone());
                        }
                        Err(e) => {
                            app.transcript
                                .push(crate::transcript::TranscriptItem::notice(
                                    format!("Failed to create worktree: {}", e),
                                    crate::transcript::NoticeLevel::Error,
                                ));
                        }
                    }
                }
                app.pop_modal();
            }
            _ => {
                app.push_modal(ModalState::WorktreeSelector {
                    worktrees,
                    selected_index,
                });
            }
        },
        ModalState::McpHubModal {
            mut servers,
            mut selected_index,
            mut view_tools,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                });
            }
            KeyCode::Down => {
                if !servers.is_empty() && selected_index + 1 < servers.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                });
            }
            KeyCode::Char(' ') => {
                if let Some(s) = servers.get_mut(selected_index) {
                    s.enabled = !s.enabled;
                    app.toggle_mcp_server_sync(&s.name);
                }
                app.push_modal(ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                });
            }
            KeyCode::Tab => {
                view_tools = !view_tools;
                app.push_modal(ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                });
            }
            _ => {
                app.push_modal(ModalState::McpHubModal {
                    servers,
                    selected_index,
                    view_tools,
                });
            }
        },
        ModalState::SkillsModal {
            skills,
            mut selected_index,
            mut search_query,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') if search_query.is_empty() => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                });
            }
            KeyCode::Down => {
                let filtered_count = skills
                    .iter()
                    .filter(|s| {
                        search_query.is_empty()
                            || s.name.to_lowercase().contains(&search_query.to_lowercase())
                            || s.description
                                .to_lowercase()
                                .contains(&search_query.to_lowercase())
                    })
                    .count();
                if filtered_count > 0 && selected_index + 1 < filtered_count {
                    selected_index += 1;
                }
                app.push_modal(ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                });
            }
            KeyCode::Backspace => {
                search_query.pop();
                selected_index = 0;
                app.push_modal(ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                });
            }
            KeyCode::Char(c) => {
                search_query.push(c);
                selected_index = 0;
                app.push_modal(ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                });
            }
            KeyCode::Enter => {
                let filtered: Vec<_> = skills
                    .iter()
                    .filter(|s| {
                        search_query.is_empty()
                            || s.name.to_lowercase().contains(&search_query.to_lowercase())
                            || s.description
                                .to_lowercase()
                                .contains(&search_query.to_lowercase())
                    })
                    .collect();
                if let Some(s) = filtered.get(selected_index) {
                    app.activate_skill(&s.name, &s.instructions);
                }
                app.pop_modal();
            }
            _ => {
                app.push_modal(ModalState::SkillsModal {
                    skills,
                    selected_index,
                    search_query,
                });
            }
        },
        ModalState::TemplatesModal {
            templates,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::TemplatesModal {
                    templates,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !templates.is_empty() && selected_index + 1 < templates.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::TemplatesModal {
                    templates,
                    selected_index,
                });
            }
            KeyCode::Enter => {
                if let Some(tmpl) = templates.get(selected_index) {
                    if let Some(mode) = tmpl.config.collaboration_mode {
                        app.collaboration_mode = mode;
                        app.bottom_pane.collaboration_mode = mode;
                    }
                    if let Some(ref model) = tmpl.config.model
                        && let Err(e) = app.switch_model(model)
                    {
                        app.provider_status_message =
                            Some(format!("Template model activation failed: {e}"));
                    }
                    if let Some(ref effort) = tmpl.config.reasoning_effort {
                        app.config.reasoning_effort = Some(effort.clone());
                        app.bottom_pane.reasoning_effort = Some(effort.clone());
                    }
                    if let Some(ref prompt) = tmpl.config.initial_prompt {
                        app.composer.set_text(prompt);
                    }
                    app.refresh_system_prompt();
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            format!(
                                "Applied collaboration template '{}' ({:?})",
                                tmpl.name, tmpl.category
                            ),
                            crate::transcript::NoticeLevel::Success,
                        ));
                    app.sync_virtualizer();
                }
                app.pop_modal();
            }
            _ => {
                app.push_modal(ModalState::TemplatesModal {
                    templates,
                    selected_index,
                });
            }
        },
        ModalState::GoalDashboardModal { goal, mut scroll } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                app.push_modal(ModalState::GoalDashboardModal { goal, scroll });
            }
            KeyCode::Down => {
                scroll = scroll.saturating_add(1);
                app.push_modal(ModalState::GoalDashboardModal { goal, scroll });
            }
            KeyCode::Char('p') => {
                app.execute_slash_command("/goal pause");
                // closed (already popped)
            }
            KeyCode::Char('r') => {
                app.execute_slash_command("/goal resume");
                // closed (already popped)
            }
            KeyCode::Char('c') => {
                app.execute_slash_command("/goal clear");
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::GoalDashboardModal { goal, scroll });
            }
        },
        ModalState::SubagentsModal {
            subagents,
            mut selected_index,
            recent_posts,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::SubagentsModal {
                    subagents,
                    selected_index,
                    recent_posts,
                });
            }
            KeyCode::Down => {
                if !subagents.is_empty() && selected_index + 1 < subagents.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::SubagentsModal {
                    subagents,
                    selected_index,
                    recent_posts,
                });
            }
            KeyCode::Enter | KeyCode::Char('s') => {
                if let Some((child_id, _role)) = subagents.get(selected_index) {
                    app.switch_subagent_view(Some(child_id.clone()));
                }
                app.pop_modal();
            }
            KeyCode::Char('k') => {
                if let Some((child_id, role)) = subagents.get(selected_index) {
                    let _ = app.terminate_subagent(child_id, role);
                }
                app.pop_modal();
            }
            KeyCode::Char('b') => {
                // closed (already popped)
                app.execute_slash_command("/board");
            }
            _ => {
                app.push_modal(ModalState::SubagentsModal {
                    subagents,
                    selected_index,
                    recent_posts,
                });
            }
        },
        ModalState::TurnScrubber {
            turns,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::TurnScrubber {
                    turns,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !turns.is_empty() && selected_index + 1 < turns.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::TurnScrubber {
                    turns,
                    selected_index,
                });
            }
            KeyCode::Enter => {
                if let Some(turn) = turns.get(selected_index) {
                    let turn_idx = turn.turn_index;
                    let prompt = turn.user_prompt.clone();
                    let chk = turn.checkpoint_id.clone();
                    // closed (already popped)
                    app.apply_backtrack_turn(turn_idx, prompt, chk);
                } else {
                    // closed (already popped)
                }
            }
            _ => {
                app.push_modal(ModalState::TurnScrubber {
                    turns,
                    selected_index,
                });
            }
        },
        ModalState::ContextManagerModal {
            mut pinned_items,
            world_sections,
            mut selected_index,
            mut active_tab,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Tab | KeyCode::Right => {
                active_tab = (active_tab + 1) % 3;
                selected_index = 0;
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::BackTab | KeyCode::Left => {
                active_tab = if active_tab == 0 { 2 } else { active_tab - 1 };
                selected_index = 0;
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Char('1') => {
                active_tab = 0;
                selected_index = 0;
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Char('2') => {
                active_tab = 1;
                selected_index = 0;
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Char('3') => {
                active_tab = 2;
                selected_index = 0;
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Down => {
                let max_items = if active_tab == 0 {
                    pinned_items.len()
                } else {
                    world_sections.len()
                };
                if max_items > 0 && selected_index + 1 < max_items {
                    selected_index += 1;
                }
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Char('u') | KeyCode::Char('d') => {
                if active_tab == 0 && selected_index < pinned_items.len() {
                    let item = pinned_items.remove(selected_index);
                    app.pinned_context.retain(|p| p.path != item.path);
                    app.refresh_system_prompt();
                    if selected_index >= pinned_items.len() && !pinned_items.is_empty() {
                        selected_index = pinned_items.len() - 1;
                    }
                }
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
            KeyCode::Char('c') => {
                app.execute_slash_command("/context compact");
                // closed (already popped)
            }
            KeyCode::Char('r') => {
                app.execute_slash_command("/context refresh");
                // closed (already popped)
            }
            _ => {
                app.push_modal(ModalState::ContextManagerModal {
                    pinned_items,
                    world_sections,
                    selected_index,
                    active_tab,
                });
            }
        },
        ModalState::PersonaModal {
            personas,
            mut selected_index,
        } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // closed (already popped)
            }
            KeyCode::Up => {
                selected_index = selected_index.saturating_sub(1);
                app.push_modal(ModalState::PersonaModal {
                    personas,
                    selected_index,
                });
            }
            KeyCode::Down => {
                if !personas.is_empty() && selected_index + 1 < personas.len() {
                    selected_index += 1;
                }
                app.push_modal(ModalState::PersonaModal {
                    personas,
                    selected_index,
                });
            }
            KeyCode::Enter => {
                if let Some(persona) = personas.get(selected_index) {
                    app.activate_persona(
                        &persona.name,
                        &persona.description,
                        persona.prompt_directive,
                    );
                }
                app.pop_modal();
            }
            _ => {
                app.push_modal(ModalState::PersonaModal {
                    personas,
                    selected_index,
                });
            }
        },
        ModalState::CodeModeScratchpad(mut sp) => {
            let is_eval_trigger = key.code == KeyCode::F(5)
                || (key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('e')
                    ));

            if is_eval_trigger {
                app.push_modal(ModalState::CodeModeScratchpad(sp));
                app.eval_code_mode_scratchpad();
                return;
            }

            match key.code {
                KeyCode::Esc => {
                    // closed (already popped)
                }
                KeyCode::Tab => {
                    sp.focus = match sp.focus {
                        crate::modals::ScratchpadFocus::Editor => {
                            crate::modals::ScratchpadFocus::Viewer
                        }
                        crate::modals::ScratchpadFocus::Viewer => {
                            crate::modals::ScratchpadFocus::Editor
                        }
                    };
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Char('1') if sp.focus == crate::modals::ScratchpadFocus::Viewer => {
                    sp.active_tab = 0;
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Char('2') if sp.focus == crate::modals::ScratchpadFocus::Viewer => {
                    sp.active_tab = 1;
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Char('3') if sp.focus == crate::modals::ScratchpadFocus::Viewer => {
                    sp.active_tab = 2;
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Char('c') if sp.focus == crate::modals::ScratchpadFocus::Viewer => {
                    sp.output_lines.clear();
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Up => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.cursor_line = sp.cursor_line.saturating_sub(1);
                    } else if sp.active_tab == 0 {
                        sp.scroll_output = sp.scroll_output.saturating_sub(1);
                    } else if sp.active_tab == 1 {
                        sp.scroll_dts = sp.scroll_dts.saturating_sub(1);
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Down => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        let line_count = sp.code.lines().count().max(1);
                        if sp.cursor_line + 1 < line_count {
                            sp.cursor_line += 1;
                        }
                    } else if sp.active_tab == 0 {
                        sp.scroll_output = sp.scroll_output.saturating_add(1);
                    } else if sp.active_tab == 1 {
                        sp.scroll_dts = sp.scroll_dts.saturating_add(1);
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Left => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.cursor_col = sp.cursor_col.saturating_sub(1);
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Right => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.cursor_col = sp.cursor_col.saturating_add(1);
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Home => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.cursor_col = 0;
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::End => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        let line_len = sp
                            .code
                            .lines()
                            .nth(sp.cursor_line)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        sp.cursor_col = line_len;
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Enter => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.insert_newline();
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Backspace => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.delete_backspace();
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                KeyCode::Char(c) => {
                    if sp.focus == crate::modals::ScratchpadFocus::Editor {
                        sp.insert_char(c);
                    }
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
                _ => {
                    app.push_modal(ModalState::CodeModeScratchpad(sp));
                }
            }
        }
        ModalState::RolloutJournal(mut journal) => {
            let filtered = journal.filtered_indices();
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    // closed (already popped)
                }
                KeyCode::Up => {
                    journal.selected_index = journal.selected_index.saturating_sub(1);
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Down => {
                    if !filtered.is_empty() && journal.selected_index + 1 < filtered.len() {
                        journal.selected_index += 1;
                    }
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    journal.view_json = !journal.view_json;
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Char('a') => {
                    journal.filter = "all".to_string();
                    journal.selected_index = 0;
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Char('p') => {
                    journal.filter = "prompts".to_string();
                    journal.selected_index = 0;
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Char('t') => {
                    journal.filter = "tools".to_string();
                    journal.selected_index = 0;
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::Char('c') => {
                    journal.filter = "checkpoints".to_string();
                    journal.selected_index = 0;
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::PageUp => {
                    journal.scroll_detail = journal.scroll_detail.saturating_sub(10);
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                KeyCode::PageDown => {
                    journal.scroll_detail = journal.scroll_detail.saturating_add(10);
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
                _ => {
                    app.push_modal(ModalState::RolloutJournal(journal));
                }
            }
        }
        ModalState::SelectPicker(mut picker) => match key.code {
            KeyCode::Esc => {
                // Closed (already popped)
            }
            KeyCode::Up => {
                picker.select_prev();
                app.push_modal(ModalState::SelectPicker(picker));
            }
            KeyCode::Down => {
                picker.select_next();
                app.push_modal(ModalState::SelectPicker(picker));
            }
            KeyCode::Backspace => {
                picker.on_backspace();
                app.push_modal(ModalState::SelectPicker(picker));
            }
            KeyCode::Enter => {
                if let Some(item) = picker.selected_item() {
                    let item_id = item.id.clone();
                    app.execute_slash_command(&format!("/{}", item_id));
                }
            }
            KeyCode::Char(c) => {
                picker.on_char(c);
                app.push_modal(ModalState::SelectPicker(picker));
            }
            _ => {
                app.push_modal(ModalState::SelectPicker(picker));
            }
        },
    }
}

pub fn handle_global_shortcut_event(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    // Model picker triggers (Ctrl+M / Alt+M / F2):
    let is_model_trigger = (key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M')))
        || (key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M')))
        || key.code == KeyCode::F(2);

    if is_model_trigger {
        app.push_modal(ModalState::model_picker(
            app.available_models.clone(),
            &app.active_model,
        ));
        return true;
    }

    // 1. Function keys F1 for Help, F12 for inspector toggle, F3..F10 for screens
    if key.code == KeyCode::F(1) {
        app.push_modal(ModalState::help());
        return true;
    }
    if key.code == KeyCode::F(12) {
        app.toggle_inspector();
        return true;
    }

    match key.code {
        KeyCode::F(3) => {
            app.switch_screen(crate::screens::Screen::Models);
            return true;
        }
        KeyCode::F(4) => {
            app.switch_screen(crate::screens::Screen::Providers);
            return true;
        }
        KeyCode::F(5) => {
            app.switch_screen(crate::screens::Screen::Agents);
            return true;
        }
        KeyCode::F(6) => {
            app.switch_screen(crate::screens::Screen::Workspace);
            return true;
        }
        KeyCode::F(7) => {
            app.switch_screen(crate::screens::Screen::SourceControl);
            return true;
        }
        KeyCode::F(8) => {
            app.switch_screen(crate::screens::Screen::ToolsIntegrations);
            return true;
        }
        KeyCode::F(9) => {
            app.switch_screen(crate::screens::Screen::Settings);
            return true;
        }
        KeyCode::F(10) => {
            app.switch_screen(crate::screens::Screen::Diagnostics);
            return true;
        }
        KeyCode::F(12) => {
            app.toggle_inspector();
            return true;
        }
        _ => {}
    }

    // 2. Alt+1..Alt+9, Alt+0 for direct screen switching
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('1') => {
                app.switch_screen(crate::screens::Screen::Chat);
                return true;
            }
            KeyCode::Char('2') => {
                app.switch_screen(crate::screens::Screen::Sessions);
                return true;
            }
            KeyCode::Char('3') => {
                app.switch_screen(crate::screens::Screen::Models);
                return true;
            }
            KeyCode::Char('4') => {
                app.switch_screen(crate::screens::Screen::Providers);
                return true;
            }
            KeyCode::Char('5') => {
                app.switch_screen(crate::screens::Screen::Agents);
                return true;
            }
            KeyCode::Char('6') => {
                app.switch_screen(crate::screens::Screen::Workspace);
                return true;
            }
            KeyCode::Char('7') => {
                app.switch_screen(crate::screens::Screen::SourceControl);
                return true;
            }
            KeyCode::Char('8') => {
                app.switch_screen(crate::screens::Screen::ToolsIntegrations);
                return true;
            }
            KeyCode::Char('9') => {
                app.switch_screen(crate::screens::Screen::Settings);
                return true;
            }
            KeyCode::Char('0') => {
                app.switch_screen(crate::screens::Screen::Diagnostics);
                return true;
            }
            KeyCode::Char('[') => {
                app.resize_panel_narrower();
                return true;
            }
            KeyCode::Char(']') => {
                app.resize_panel_wider();
                return true;
            }
            _ => {}
        }
    }

    // 3. Global Control combinations
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let _ = app.create_new_session(None);
                return true;
            }
            KeyCode::Char('[') => {
                app.cycle_panel_tab_prev();
                return true;
            }
            KeyCode::Char(']') => {
                app.cycle_panel_tab_next();
                return true;
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                app.toggle_inspector();
                return true;
            }
            KeyCode::Char('p') => {
                app.open_command_palette();
                return true;
            }
            KeyCode::Char('s') => {
                app.push_modal(ModalState::settings());
                return true;
            }
            KeyCode::Char('d') => {
                app.switch_screen(crate::screens::Screen::SourceControl);
                return true;
            }
            KeyCode::Char('r') => {
                let threads = app.storage.list_threads(100).unwrap_or_default();
                let mut picker = crate::resume_picker::ResumePickerState::new(threads);
                picker.active_session_id = Some(app.session_id.clone());
                app.push_modal(ModalState::ResumePicker(picker));
                return true;
            }
            KeyCode::Char('h') => {
                app.push_modal(ModalState::help());
                return true;
            }
            KeyCode::Char('c') => {
                if app.app_status() != AppStatus::Idle || app.turn_state.is_active() {
                    app.cancel_turn();
                } else if !app.composer.is_empty() {
                    app.composer.clear();
                } else {
                    // Double-press Ctrl+C quit with reminder (§7.6)
                    let now = std::time::Instant::now();
                    if let Some(last) = app.last_ctrl_c_press
                        && now.duration_since(last) < std::time::Duration::from_secs(2)
                    {
                        app.should_quit = true;
                        return true;
                    }
                    app.last_ctrl_c_press = Some(now);
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            "Press Ctrl+C again within 2 seconds to quit.",
                            crate::transcript::NoticeLevel::Info,
                        ));
                    app.sync_virtualizer();
                }
                return true;
            }
            KeyCode::Char('a') => {
                app.push_modal(ModalState::AnalyticsDashboard);
                return true;
            }
            KeyCode::Char('q') => {
                app.should_quit = true;
                return true;
            }
            KeyCode::Char('y') => {
                if let Some(block) = app.transcript_virtualizer.find_last_code_block() {
                    let outcome = crate::clipboard::copy_to_clipboard(&block.code);
                    let (notice_msg, notice_level) = match outcome {
                        crate::clipboard::ClipboardResult::OsClipboardSuccess => (
                            format!(
                                "Copied {} lines of {} code to system clipboard.",
                                block.line_count,
                                block.language.as_deref().unwrap_or("snippet")
                            ),
                            crate::transcript::NoticeLevel::Success,
                        ),
                        crate::clipboard::ClipboardResult::Osc52EmittedOnly => (
                            format!(
                                "Emitted OSC 52 sequence for {} lines of {} code (system clipboard CLI unavailable).",
                                block.line_count,
                                block.language.as_deref().unwrap_or("snippet")
                            ),
                            crate::transcript::NoticeLevel::Info,
                        ),
                        crate::clipboard::ClipboardResult::WriteFailed
                        | crate::clipboard::ClipboardResult::Unavailable => (
                            format!(
                                "Failed to copy {} lines of {} code (clipboard unavailable).",
                                block.line_count,
                                block.language.as_deref().unwrap_or("snippet")
                            ),
                            crate::transcript::NoticeLevel::Warning,
                        ),
                    };
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            notice_msg,
                            notice_level,
                        ));
                    app.sync_virtualizer();
                } else {
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            "No code block found in transcript to copy.",
                            crate::transcript::NoticeLevel::Info,
                        ));
                    app.sync_virtualizer();
                }
                return true;
            }
            KeyCode::Char('w') => {
                if let Ok(mgr) = m31a_core::worktree::WorktreeManager::new() {
                    match mgr.list_worktrees(&app.cwd) {
                        Ok(worktrees) => {
                            let current_idx = worktrees
                                .iter()
                                .position(|w| w.path == app.cwd)
                                .unwrap_or(0);
                            app.push_modal(ModalState::WorktreeSelector {
                                worktrees,
                                selected_index: current_idx,
                            });
                        }
                        Err(e) => {
                            app.transcript
                                .push(crate::transcript::TranscriptItem::notice(
                                    format!("Worktree error: {}", e),
                                    crate::transcript::NoticeLevel::Error,
                                ));
                        }
                    }
                }
                return true;
            }
            KeyCode::Char('l') => {
                app.transcript.clear();
                app.scroll_offset = 0;
                app.sync_virtualizer();
                return true;
            }
            KeyCode::Char('o') => {
                let main_rs = app.cwd.join("src").join("main.rs");
                let lib_rs = app.cwd.join("src").join("lib.rs");
                let target_file = if main_rs.exists() {
                    main_rs
                } else if lib_rs.exists() {
                    lib_rs
                } else {
                    app.cwd.clone()
                };

                if target_file.is_file()
                    && let Ok(content) = std::fs::read_to_string(&target_file)
                {
                    let rel_path = target_file.strip_prefix(&app.cwd).unwrap_or(&target_file);
                    let symbols = m31a_core::parse_symbols_from_source(rel_path, &content);
                    app.push_modal(ModalState::symbol_outline(
                        rel_path.display().to_string(),
                        symbols,
                    ));
                }
                return true;
            }
            KeyCode::Char('t') => {
                app.transcript_virtualizer.toggle_all_collapsed();
                for item in app.transcript.iter_mut() {
                    if let crate::transcript::TranscriptItem::ToolCall(entry) = item {
                        entry.toggle_collapsed();
                    }
                }
                return true;
            }
            KeyCode::Char('k') => {
                let durations = app.last_turn_critical_path.clone().unwrap_or_default();
                let intervals = app.direct_tool_timings.completed_work();
                app.push_modal(ModalState::critical_path_modal(durations, intervals));
                return true;
            }
            KeyCode::Char('e') => {
                let locks = m31a_core::ExecutionLockManager::global().active_locks();
                app.push_modal(ModalState::active_locks(locks));
                return true;
            }
            _ => {}
        }
    }

    false
}

pub fn handle_composer_key_event(app: &mut App, key: crossterm::event::KeyEvent) {
    // Transcript scrolling & navigation
    match key.code {
        KeyCode::PageUp => {
            app.scroll_up(8);
            return;
        }
        KeyCode::PageDown => {
            app.scroll_down(8);
            return;
        }
        KeyCode::Home
            if key.modifiers.contains(KeyModifiers::CONTROL) || app.composer.is_empty() =>
        {
            app.scroll_to_top();
            return;
        }
        KeyCode::End
            if key.modifiers.contains(KeyModifiers::CONTROL) || app.composer.is_empty() =>
        {
            app.scroll_to_bottom();
            return;
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
            app.switch_active_subagent(false);
            return;
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
            app.switch_active_subagent(true);
            return;
        }
        KeyCode::Esc => {
            if app.composer.is_empty()
                && app.bottom_pane.mode == crate::bottom_pane::BottomPaneMode::InputNormal
            {
                if !app.open_turn_scrubber() {
                    app.bottom_pane.on_escape();
                }
            } else {
                app.bottom_pane.on_escape();
            }
            return;
        }
        _ => {}
    }

    // Bottom pane input routing
    match key.code {
        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let msg = app.composer.buffer.trim().to_string();
                if !msg.is_empty()
                    && matches!(
                        app.app_status(),
                        AppStatus::Streaming(_) | AppStatus::ExecutingTool { .. }
                    )
                {
                    app.queued_prompts.push_back(msg);
                    app.composer.clear();
                    let q_len = app.queued_prompts.len();
                    app.transcript
                        .push(crate::transcript::TranscriptItem::notice(
                            format!(
                                "Prompt queued (#{q_len} in queue, will auto-run when turn settles)"
                            ),
                            crate::transcript::NoticeLevel::Info,
                        ));
                    app.sync_virtualizer();
                    return;
                } else if !msg.is_empty() && matches!(app.app_status(), AppStatus::Idle) {
                    if let Some(submission) = app.bottom_pane.on_enter(&mut app.composer) {
                        match submission {
                            crate::bottom_pane::BottomPaneSubmit::Message(m) => {
                                app.submit_prompt(m);
                            }
                            crate::bottom_pane::BottomPaneSubmit::SlashCommand(c) => {
                                app.execute_slash_command(&c);
                            }
                            crate::bottom_pane::BottomPaneSubmit::ModelSelected(slug) => {
                                if let Err(e) = app.switch_model(&slug) {
                                    app.provider_status_message =
                                        Some(format!("Failed to activate model: {e}"));
                                }
                            }
                        }
                    }
                    return;
                }
            }

            if key.modifiers.contains(KeyModifiers::SHIFT)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                app.bottom_pane.on_char('\n', &mut app.composer);
            } else if let Some(submission) = app.bottom_pane.on_enter(&mut app.composer) {
                match submission {
                    crate::bottom_pane::BottomPaneSubmit::Message(msg) => {
                        app.submit_prompt(msg);
                    }
                    crate::bottom_pane::BottomPaneSubmit::SlashCommand(cmd) => {
                        app.execute_slash_command(&cmd);
                    }
                    crate::bottom_pane::BottomPaneSubmit::ModelSelected(slug) => {
                        if let Err(e) = app.switch_model(&slug) {
                            app.provider_status_message =
                                Some(format!("Failed to activate model: {e}"));
                        }
                    }
                }
            }
        }
        KeyCode::Tab => {
            if !app.bottom_pane.on_tab(&mut app.composer) {
                // If not in autocomplete popup, Tab toggles between BUILD and PLAN mode
                app.toggle_collaboration_mode();
            }
        }
        KeyCode::BackTab => {
            app.toggle_collaboration_mode();
        }
        KeyCode::Backspace => {
            app.bottom_pane.on_backspace(&mut app.composer);
        }
        KeyCode::Delete => {
            app.bottom_pane.on_delete(&mut app.composer);
        }
        KeyCode::Left => {
            app.bottom_pane.move_cursor_left(&mut app.composer);
        }
        KeyCode::Right => {
            app.bottom_pane.move_cursor_right(&mut app.composer);
        }
        KeyCode::Home => {
            app.composer.move_cursor_home();
        }
        KeyCode::End => {
            app.composer.move_cursor_end();
        }
        KeyCode::Up => {
            if !app.bottom_pane.on_up() {
                app.composer.history_prev();
            }
        }
        KeyCode::Down => {
            if !app.bottom_pane.on_down() {
                app.composer.history_next();
            }
        }
        KeyCode::Char('?') if app.composer.is_empty() => {
            app.push_modal(ModalState::help());
        }
        KeyCode::Char('i') if app.composer.is_empty() => {
            let mut last_error_desc = None;
            for item in app.transcript.iter().rev() {
                match item {
                    crate::transcript::TranscriptItem::Notice { text, level }
                        if *level == crate::transcript::NoticeLevel::Error =>
                    {
                        last_error_desc = Some(text.clone());
                        break;
                    }
                    crate::transcript::TranscriptItem::ToolCall(entry)
                        if entry.success == Some(false) =>
                    {
                        let out_snippet =
                            entry.output.as_deref().unwrap_or("tool execution failed");
                        let err_line = out_snippet.lines().last().unwrap_or(out_snippet);
                        last_error_desc = Some(format!("tool {} failed: {}", entry.name, err_line));
                        break;
                    }
                    _ => {}
                }
            }
            if let Some(err) = last_error_desc {
                let debug_prompt = format!("Investigate and fix failure: {err}");
                app.composer.set_text(&debug_prompt);
                app.transcript.push(crate::transcript::TranscriptItem::notice(
                    "Populated composer with error investigation prompt. Press Enter to execute.",
                    crate::transcript::NoticeLevel::Info,
                ));
                app.sync_virtualizer();
            } else {
                app.bottom_pane.on_char('i', &mut app.composer);
            }
        }
        KeyCode::Char(c) => {
            app.bottom_pane.on_char(c, &mut app.composer);
        }
        _ => {}
    }
}

fn filter_palette_items(query: &str, items: &[crate::modals::CommandPaletteItem]) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..items.len()).collect();
    }
    let q = query.to_lowercase();
    let mut scored: Vec<(usize, i32)> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let title = item.title.to_lowercase();
            let desc = item.description.to_lowercase();
            let cat = item.category.to_lowercase();
            let short = item.shortcut.as_deref().unwrap_or("").to_lowercase();

            let mut score = 0;
            if title.starts_with(&q) {
                score += 100;
            } else if title.contains(&q) {
                score += 50;
            } else if desc.contains(&q) {
                score += 30;
            } else if cat.contains(&q) {
                score += 20;
            } else if short.contains(&q) {
                score += 15;
            } else {
                return None;
            }
            Some((idx, score))
        })
        .collect();

    scored.sort_by_key(|a| std::cmp::Reverse(a.1));
    scored.into_iter().map(|(idx, _)| idx).collect()
}

pub fn execute_palette_action(app: &mut App, action_id: &str) {
    match action_id {
        "nav_chat" => {
            app.switch_screen(crate::screens::Screen::Chat);
        }
        "nav_sessions" => {
            app.switch_screen(crate::screens::Screen::Sessions);
        }
        "nav_models" => {
            app.switch_screen(crate::screens::Screen::Models);
        }
        "nav_providers" => {
            app.switch_screen(crate::screens::Screen::Providers);
        }
        "nav_agents" => {
            app.switch_screen(crate::screens::Screen::Agents);
        }
        "nav_workspace" => {
            app.switch_screen(crate::screens::Screen::Workspace);
        }
        "nav_source_control" => {
            app.switch_screen(crate::screens::Screen::SourceControl);
        }
        "nav_tools" => {
            app.switch_screen(crate::screens::Screen::ToolsIntegrations);
        }
        "nav_settings" => {
            app.switch_screen(crate::screens::Screen::Settings);
        }
        "nav_diagnostics" => {
            app.switch_screen(crate::screens::Screen::Diagnostics);
        }
        "session_new" => {
            let _ = app.create_new_session(None);
        }
        "worktrees" => {
            if let Ok(mgr) = m31a_core::worktree::WorktreeManager::new() {
                if let Ok(worktrees) = mgr.list_worktrees(&app.cwd) {
                    let current_idx = worktrees
                        .iter()
                        .position(|w| w.path == app.cwd)
                        .unwrap_or(0);
                    app.push_modal(ModalState::WorktreeSelector {
                        worktrees,
                        selected_index: current_idx,
                    });
                }
            }
        }
        "cycle_reasoning" => {
            let next_effort = match app.nim_client.reasoning_effort().as_deref() {
                None => Some("low".to_string()),
                Some("low") => Some("medium".to_string()),
                Some("medium") => Some("high".to_string()),
                _ => None,
            };
            app.nim_client = app
                .nim_client
                .clone()
                .with_reasoning_effort(next_effort.clone());
            app.bottom_pane.reasoning_effort = next_effort.clone();
            app.transcript
                .push(crate::transcript::TranscriptItem::notice(
                    format!(
                        "Reasoning effort set to: {}",
                        next_effort.as_deref().unwrap_or("none/standard")
                    ),
                    crate::transcript::NoticeLevel::Info,
                ));
            app.sync_virtualizer();
        }
        "test_provider" => {
            let client = app.nim_client.clone();
            let tx = app.event_tx.clone();
            tokio::spawn(async move {
                match client.test_model_availability().await {
                    Ok(_) => {
                        let _ = tx
                            .send(TuiEvent::Notice(
                                "Provider endpoint verified healthy.".to_string(),
                                crate::transcript::NoticeLevel::Success,
                            ))
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(TuiEvent::Notice(
                                format!("Provider endpoint probe failed: {e}"),
                                crate::transcript::NoticeLevel::Error,
                            ))
                            .await;
                    }
                }
            });
        }
        "panel_tab_files" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Files);
        }
        "panel_tab_git" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Git);
        }
        "panel_tab_diff" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Diff);
        }
        "panel_tab_term" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Terminal);
        }
        "panel_tab_context" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Context);
        }
        "panel_tab_activity" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Activity);
        }
        "panel_tab_subagents" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Subagents);
        }
        "panel_tab_diagnostics" => {
            app.inspector_open = true;
            app.set_panel_tab(crate::panel::PanelTab::Diagnostics);
        }
        "panel_widen" => {
            app.resize_panel_wider();
        }
        "panel_narrow" => {
            app.resize_panel_narrower();
        }
        "toggle_inspector" => {
            app.toggle_inspector();
        }
        "settings" => {
            app.switch_screen(crate::screens::Screen::Settings);
        }
        "mode_toggle" => {
            app.toggle_collaboration_mode();
        }
        "model_picker" => {
            app.switch_screen(crate::screens::Screen::Models);
        }
        "git_diff" => {
            app.switch_screen(crate::screens::Screen::SourceControl);
        }
        "resume_picker" => {
            app.switch_screen(crate::screens::Screen::Sessions);
        }
        "undo" => {
            app.execute_slash_command("/undo");
        }
        "verify" => {
            app.execute_slash_command("/verify");
        }
        "test" => {
            app.execute_slash_command("/test");
        }
        "review" => {
            app.execute_slash_command("/review");
        }
        "analytics" => {
            app.push_modal(ModalState::AnalyticsDashboard);
        }
        "checkpoints" => {
            app.execute_slash_command("/checkpoints");
        }
        "procs" => {
            app.execute_slash_command("/procs");
        }
        "locks" => {
            let locks = m31a_core::ExecutionLockManager::global().active_locks();
            app.push_modal(ModalState::active_locks(locks));
        }
        "timing" => {
            let durations = app.last_turn_critical_path.clone().unwrap_or_default();
            let intervals = app.direct_tool_timings.completed_work();
            app.push_modal(ModalState::critical_path_modal(durations, intervals));
        }
        "symbol_outline" => {
            let main_rs = app.cwd.join("src").join("main.rs");
            let lib_rs = app.cwd.join("src").join("lib.rs");
            let target_file = if main_rs.exists() {
                main_rs
            } else if lib_rs.exists() {
                lib_rs
            } else {
                app.cwd.clone()
            };

            if target_file.is_file()
                && let Ok(content) = std::fs::read_to_string(&target_file)
            {
                let rel_path = target_file.strip_prefix(&app.cwd).unwrap_or(&target_file);
                let symbols = m31a_core::parse_symbols_from_source(rel_path, &content);
                app.push_modal(ModalState::symbol_outline(
                    rel_path.display().to_string(),
                    symbols,
                ));
            }
        }
        "compact" => {
            app.execute_slash_command("/compact");
        }
        "copy_code" => {
            if let Some(block) = app.transcript_virtualizer.find_last_code_block() {
                let _ = crate::clipboard::copy_to_clipboard(&block.code);
                let msg = format!(
                    "Extracted and copied {} lines of {} code to clipboard",
                    block.line_count,
                    block.language.as_deref().unwrap_or("snippet")
                );
                app.transcript
                    .push(crate::transcript::TranscriptItem::notice(
                        msg,
                        crate::transcript::NoticeLevel::Success,
                    ));
                app.sync_virtualizer();
            }
        }
        "mcp_hub" => {
            let mcp = app.mcp_manager.clone();
            let servers = app
                .config
                .mcp_servers
                .iter()
                .map(|(name, cfg)| {
                    let is_conn = mcp.is_server_connected_sync(name);
                    let tools = mcp.get_server_tools_sync(name);
                    let tools_count = tools.len();
                    crate::modals::McpServerDisplay {
                        name: name.clone(),
                        command: cfg.command.clone(),
                        transport: cfg.transport.clone().unwrap_or_else(|| "stdio".to_string()),
                        tools_count,
                        tools,
                        enabled: is_conn,
                    }
                })
                .collect();
            app.push_modal(ModalState::mcp_hub(servers));
        }
        "skills_library" => {
            let skills = m31a_core::skills::SkillLoader::discover_skills(&app.cwd);
            app.push_modal(ModalState::skills_modal(skills));
        }
        "templates" => {
            let custom_dir = m31a_core::Config::config_dir().join("templates");
            let library = m31a_collaboration_templates::TemplateLibrary::new(custom_dir);
            let templates = library.list().into_iter().cloned().collect();
            app.push_modal(ModalState::templates_modal(templates));
        }
        "goal_dashboard" => {
            let goal = app.storage.get_goal(&app.session_id).ok().flatten();
            app.push_modal(ModalState::goal_dashboard(goal));
        }
        "subagents" => {
            let agents = app
                .storage
                .list_subagents(&app.session_id)
                .unwrap_or_default();
            let recent_posts = app
                .storage
                .list_posts("default", "dev", 10)
                .unwrap_or_default();
            app.push_modal(ModalState::subagents_modal(agents, recent_posts));
        }
        "context_mgr" => {
            let pinned = app.pinned_context.clone();
            let world_sections = app
                .world_state_digests
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            app.push_modal(ModalState::context_manager(pinned, world_sections));
        }
        "persona_picker" => {
            app.push_modal(ModalState::persona_modal(
                ModalState::default_personas(),
                None,
            ));
        }
        "fold_tools" => {
            app.transcript_virtualizer.toggle_all_collapsed();
            for item in app.transcript.iter_mut() {
                if let crate::transcript::TranscriptItem::ToolCall(entry) = item {
                    entry.toggle_collapsed();
                }
            }
        }
        "scratchpad" => {
            app.open_code_mode_scratchpad();
        }
        "rollout" => {
            app.open_rollout_journal();
        }
        "clear" => {
            app.transcript.clear();
            app.scroll_offset = 0;
            app.sync_virtualizer();
        }
        "doctor" => {
            app.execute_slash_command("/doctor");
        }
        "help" => {
            app.push_modal(ModalState::help());
        }
        _ => {}
    }
}

fn settings_items_count(tab: usize) -> usize {
    crate::mouse::settings_items_count(tab)
}

fn toggle_or_cycle_setting(app: &mut App, active_tab: usize, selected_index: usize) {
    crate::mouse::toggle_or_cycle_setting(app, active_tab, selected_index);
}

pub fn handle_screen_key_event(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.screen_filter_active {
        match key.code {
            KeyCode::Esc => {
                app.screen_filter_active = false;
                app.screen_filter.clear();
            }
            KeyCode::Enter => {
                app.screen_filter_active = false;
            }
            KeyCode::Backspace => {
                app.screen_filter.pop();
                // Update model list state for Models screen
                if app.current_screen == crate::screens::Screen::Models
                    && let Some(ref mut ms) = app.models_screen_state
                {
                    ms.set_search_query(app.screen_filter.clone());
                    app.screen_selected_index = ms.selected_index;
                }
            }
            KeyCode::Char(c) => {
                app.screen_filter.push(c);
                app.screen_selected_index = 0;
                // Update model list state for Models screen
                if app.current_screen == crate::screens::Screen::Models
                    && let Some(ref mut ms) = app.models_screen_state
                {
                    ms.set_search_query(app.screen_filter.clone());
                    app.screen_selected_index = ms.selected_index;
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            // Esc returns to Chat screen
            app.switch_screen(crate::screens::Screen::Chat);
        }
        KeyCode::Tab => {
            app.next_screen();
        }
        KeyCode::BackTab => {
            app.prev_screen();
        }
        KeyCode::Char('/') => {
            app.screen_filter_active = true;
            app.screen_filter.clear();
            app.screen_selected_index = 0;
            // Initialize/reset model list state for Models screen
            if app.current_screen == crate::screens::Screen::Models {
                let mut state = crate::model_selector::ModelListState::new(
                    app.available_models.clone(),
                    app.active_model.clone(),
                );
                state.set_search_query(String::new());
                app.models_screen_state = Some(state);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => match app.current_screen {
            crate::screens::Screen::Models => {
                if let Some(ref mut ms) = app.models_screen_state {
                    ms.select_prev();
                    app.screen_selected_index = ms.selected_index;
                }
            }
            _ => {
                if app.screen_selected_index > 0 {
                    app.screen_selected_index -= 1;
                }
            }
        },
        KeyCode::Down | KeyCode::Char('j') => match app.current_screen {
            crate::screens::Screen::Models => {
                if let Some(ref mut ms) = app.models_screen_state {
                    ms.select_next();
                    app.screen_selected_index = ms.selected_index;
                }
            }
            _ => {
                app.screen_selected_index += 1;
            }
        },
        KeyCode::PageUp => {
            if app.current_screen == crate::screens::Screen::Models
                && let Some(ref mut ms) = app.models_screen_state
            {
                ms.select_page_up();
                app.screen_selected_index = ms.selected_index;
            }
        }
        KeyCode::PageDown => {
            if app.current_screen == crate::screens::Screen::Models
                && let Some(ref mut ms) = app.models_screen_state
            {
                ms.select_page_down();
                app.screen_selected_index = ms.selected_index;
            }
        }
        KeyCode::Home => {
            if app.current_screen == crate::screens::Screen::Models
                && let Some(ref mut ms) = app.models_screen_state
            {
                ms.select_home();
                app.screen_selected_index = ms.selected_index;
            }
        }
        KeyCode::End => {
            if app.current_screen == crate::screens::Screen::Models
                && let Some(ref mut ms) = app.models_screen_state
            {
                ms.select_end();
                app.screen_selected_index = ms.selected_index;
            }
        }
        KeyCode::Char('1') => app.switch_screen(crate::screens::Screen::Chat),
        KeyCode::Char('2') => app.switch_screen(crate::screens::Screen::Sessions),
        KeyCode::Char('3') => app.switch_screen(crate::screens::Screen::Models),
        KeyCode::Char('4') => app.switch_screen(crate::screens::Screen::Providers),
        KeyCode::Char('5') => app.switch_screen(crate::screens::Screen::Agents),
        KeyCode::Char('6') => app.switch_screen(crate::screens::Screen::Workspace),
        KeyCode::Char('7') => app.switch_screen(crate::screens::Screen::SourceControl),
        KeyCode::Char('8') => app.switch_screen(crate::screens::Screen::ToolsIntegrations),
        KeyCode::Char('9') => app.switch_screen(crate::screens::Screen::Settings),
        KeyCode::Char('0') => app.switch_screen(crate::screens::Screen::Diagnostics),
        KeyCode::Char('i') => {
            app.toggle_inspector();
        }
        KeyCode::Enter => match app.current_screen {
            crate::screens::Screen::Sessions => {
                if let Ok(threads) = app.storage.list_threads(50)
                    && let Some(t) = threads.get(app.screen_selected_index)
                {
                    let tid = t.id.clone();
                    if let Ok(()) = app.resume_session(&tid) {
                        app.switch_screen(crate::screens::Screen::Chat);
                    }
                }
            }
            crate::screens::Screen::Models => {
                let model_to_switch = if let Some(ref ms) = app.models_screen_state {
                    ms.get_selected_model().map(|m| m.slug.clone())
                } else {
                    app.available_models
                        .get(app.screen_selected_index)
                        .map(|m| m.slug.clone())
                };
                if let Some(slug) = model_to_switch {
                    match app.switch_model(&slug) {
                        Ok(_) => {
                            app.provider_status_message = Some(format!("Activated model: {slug}"));
                        }
                        Err(e) => {
                            app.provider_status_message =
                                Some(format!("Failed to activate model: {e}"));
                        }
                    }
                }
            }
            crate::screens::Screen::Providers => {
                app.provider_status_message = Some("Active provider endpoint updated".to_string());
            }
            crate::screens::Screen::Settings => {
                match app.screen_selected_index % 5 {
                    0 => app.config.tui.vim_mode_default = !app.config.tui.vim_mode_default,
                    1 => app.config.tui.mouse_enabled = !app.config.tui.mouse_enabled,
                    2 => app.config.tui.animations = !app.config.tui.animations,
                    3 => app.auto_scroll = !app.auto_scroll,
                    4 => app.toggle_collaboration_mode(),
                    _ => {}
                }
                let _ = app.config.save();
            }
            _ => {}
        },
        KeyCode::Char('t') => match app.current_screen {
            crate::screens::Screen::Providers => {
                let client = app.nim_client.clone();
                let event_tx = app.event_tx.clone();
                app.provider_status_message =
                    Some("Checking provider endpoint connectivity...".to_string());
                tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    let res = client.check_connection().await;
                    let elapsed = start.elapsed();
                    let msg = match res {
                        Ok(()) => format!(
                            "Endpoint connection verified: 200 OK (latency: {}ms)",
                            elapsed.as_millis()
                        ),
                        Err(e) => format!(
                            "Endpoint connection failed ({}ms): {e}",
                            elapsed.as_millis()
                        ),
                    };
                    let _ = event_tx
                        .send(crate::app::TuiEvent::ProviderStatus(msg))
                        .await;
                });
            }
            crate::screens::Screen::Models => {
                let client = app.nim_client.clone();
                let event_tx = app.event_tx.clone();
                let model = if let Some(ref ms) = app.models_screen_state {
                    ms.get_selected_model().map(|m| m.slug.clone())
                } else {
                    app.available_models
                        .get(app.screen_selected_index)
                        .map(|m| m.slug.clone())
                }
                .unwrap_or_else(|| app.active_model.clone());
                app.provider_status_message =
                    Some(format!("Testing inference for model '{model}'..."));
                tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    let res = client.test_model_availability().await;
                    let elapsed = start.elapsed();
                    let msg = match res {
                        Ok(()) => format!(
                            "Model healthcheck: responsive ({model}, latency: {}ms)",
                            elapsed.as_millis()
                        ),
                        Err(e) => format!(
                            "Model healthcheck failed ({model}, {}ms): {e}",
                            elapsed.as_millis()
                        ),
                    };
                    let _ = event_tx
                        .send(crate::app::TuiEvent::ProviderStatus(msg))
                        .await;
                });
            }
            crate::screens::Screen::Workspace => {
                if let Ok(mut store) = app.trust_store.lock() {
                    let _ = store.trust(&app.cwd, m31a_trust::TrustLevel::SessionOnly);
                    app.provider_status_message =
                        Some("Workspace session trust granted".to_string());
                }
            }
            crate::screens::Screen::Agents => {
                if let Some(tab) = app.bottom_pane.subagent_tabs.first().cloned() {
                    let _ = app.terminate_subagent(&tab.thread_id, &tab.role);
                    app.provider_status_message = Some("Subagent tree terminated".to_string());
                } else {
                    app.provider_status_message = Some("No child agents active".to_string());
                }
            }
            _ => {}
        },
        KeyCode::Char('d') => {
            if app.current_screen == crate::screens::Screen::SourceControl {
                let diff_out = std::process::Command::new("git")
                    .args(["diff", "HEAD"])
                    .current_dir(&app.cwd)
                    .output();
                if let Ok(out) = diff_out {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    let state =
                        m31a_diff::DiffViewerState::from_unified_patch("Git Diff (HEAD)", &text);
                    app.push_modal(crate::modals::ModalState::InteractiveDiff(state));
                }
            } else if app.current_screen == crate::screens::Screen::Sessions
                && let Ok(threads) = app.storage.list_threads(50)
                && let Some(t) = threads.get(app.screen_selected_index)
            {
                let _ = app.storage.delete_thread(&t.id);
                app.provider_status_message = Some(format!("Deleted session {}", t.id));
            }
        }
        _ => {}
    }
}
