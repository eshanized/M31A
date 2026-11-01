//! Interactive Session Resume Picker
//!
//! Interactive split-view modal for browsing, searching, previewing, and resuming previous sessions.
//! Provides an interactive split-view modal for browsing, searching, previewing,
//! and resuming past sessions, as well as forking, archiving, or deleting them.

use m31a_storage::db::ThreadRecord;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

#[derive(Debug, Clone)]
pub struct ResumePickerState {
    pub all_threads: Vec<ThreadRecord>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub search_query: String,
    pub preview_summary: Option<String>,
    pub active_session_id: Option<String>,
}

fn format_relative_time(epoch_sec: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = now.saturating_sub(epoch_sec);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

impl ResumePickerState {
    pub fn new(threads: Vec<ThreadRecord>) -> Self {
        let indices = (0..threads.len()).collect();
        let mut state = Self {
            all_threads: threads,
            filtered_indices: indices,
            selected_index: 0,
            search_query: String::new(),
            preview_summary: None,
            active_session_id: None,
        };
        state.update_preview();
        state
    }

    pub fn selected_thread(&self) -> Option<&ThreadRecord> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.all_threads.get(idx))
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.update_preview();
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered_indices.is_empty()
            && self.selected_index + 1 < self.filtered_indices.len()
        {
            self.selected_index += 1;
            self.update_preview();
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.search_query.push(c);
        self.refilter();
    }

    pub fn pop_char(&mut self) {
        self.search_query.pop();
        self.refilter();
    }

    pub fn refilter(&mut self) {
        let q = self.search_query.to_lowercase();
        self.filtered_indices = self
            .all_threads
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if q.is_empty() {
                    return true;
                }
                t.title.to_lowercase().contains(&q)
                    || t.id.to_lowercase().contains(&q)
                    || t.model.to_lowercase().contains(&q)
                    || t.cwd.to_lowercase().contains(&q)
            })
            .map(|(idx, _)| idx)
            .collect();

        if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len().saturating_sub(1);
        }
        self.update_preview();
    }

    fn update_preview(&mut self) {
        if let Some(t) = self.selected_thread() {
            let date_str = chrono::DateTime::from_timestamp(t.updated_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            self.preview_summary = Some(format!(
                "Session Details:\n\
                - ID: {}\n\
                - Title: {}\n\
                - Model: {}\n\
                - Workspace: {}\n\
                - Sandbox: {}\n\
                - Approval: {}\n\
                - Archived: {}\n\
                - Last Updated: {}\n\
                \n\
                Actions:\n\
                [Enter] Resume this thread\n\
                [f]     Fork into new branch\n\
                [a]     Toggle archive\n\
                [d]     Delete thread\n\
                [Esc]   Cancel and return",
                t.id,
                t.title,
                t.model,
                t.cwd,
                t.sandbox_policy,
                t.approval_mode,
                t.is_archived,
                date_str
            ));
        } else {
            self.preview_summary = Some("No matching sessions found.".to_string());
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        let root_block = Block::default()
            .title(" Resume Past Session (Codex Session Picker) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner_area = root_block.inner(area);
        f.render_widget(root_block, area);

        // Split vertically: top search query (3 lines), middle content (split horizontal), bottom hints (1 line)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search bar
                Constraint::Min(5),    // Session list + preview
                Constraint::Length(1), // Hint bar
            ])
            .split(inner_area);

        // 1. Search Bar
        let search_text = format!("Search query: {}█", self.search_query);
        let search_para = Paragraph::new(search_text)
            .block(Block::default().borders(Borders::ALL).title(" Filter "))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(search_para, main_chunks[0]);

        // 2. Split Middle: Left session list (50%), Right preview pane (50%)
        let middle_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_chunks[1]);

        // Left List
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .enumerate()
            .map(|(display_idx, &real_idx)| {
                let t = &self.all_threads[real_idx];
                let is_selected = display_idx == self.selected_index;

                let title_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if t.is_archived {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };

                let is_active = self.active_session_id.as_deref() == Some(&t.id);
                let active_badge = if is_active { " [● ACTIVE]" } else { "" };
                let archive_badge = if t.is_archived { " [ARCHIVED]" } else { "" };
                let fork_badge = if t.parent_thread_id.is_some() {
                    " [FORK]"
                } else {
                    ""
                };
                let rel_time = format_relative_time(t.updated_at);
                let line_text = format!(
                    "{} {}{} · {} · {}{}{}",
                    if is_selected { "▶" } else { " " },
                    t.title,
                    active_badge,
                    t.model,
                    rel_time,
                    fork_badge,
                    archive_badge
                );

                ListItem::new(Line::from(vec![Span::styled(line_text, title_style)]))
            })
            .collect();

        let list_widget = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Sessions ({}) ", self.filtered_indices.len())),
        );
        f.render_widget(list_widget, middle_chunks[0]);

        // Right Preview
        let preview_text = self.preview_summary.as_deref().unwrap_or("");
        let preview_para = Paragraph::new(preview_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Session Summary "),
            )
            .style(Style::default().fg(Color::Gray));
        f.render_widget(preview_para, middle_chunks[1]);

        // 3. Bottom Hints
        let hints = Line::from(vec![
            Span::styled(
                " [↑/↓] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Navigate · "),
            Span::styled(
                "[Enter] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Switch · "),
            Span::styled(
                "[n] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("New · "),
            Span::styled(
                "[f] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Fork · "),
            Span::styled(
                "[a] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Archive · "),
            Span::styled(
                "[d] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Delete · "),
            Span::styled(
                "[Esc] ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Close"),
        ]);
        f.render_widget(Paragraph::new(hints), main_chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resume_picker_filtering_and_navigation() {
        let threads = vec![
            ThreadRecord {
                id: "t1".to_string(),
                title: "Refactor database".to_string(),
                model: "meta/llama-3.3-70b-instruct".to_string(),
                cwd: "/repo".to_string(),
                sandbox_policy: "workspace-write".to_string(),
                approval_mode: "on-request".to_string(),
                is_archived: false,
                parent_thread_id: None,
                section_id: None,
                created_at: 100,
                updated_at: 200,
            },
            ThreadRecord {
                id: "t2".to_string(),
                title: "Build UI layout".to_string(),
                model: "qwen/qwen2.5-coder-32b-instruct".to_string(),
                cwd: "/repo/ui".to_string(),
                sandbox_policy: "workspace-write".to_string(),
                approval_mode: "on-request".to_string(),
                is_archived: false,
                parent_thread_id: None,
                section_id: None,
                created_at: 150,
                updated_at: 250,
            },
        ];

        let mut picker = ResumePickerState::new(threads);
        assert_eq!(picker.filtered_indices.len(), 2);
        assert_eq!(picker.selected_thread().unwrap().title, "Refactor database");

        // Navigate down
        picker.move_down();
        assert_eq!(picker.selected_thread().unwrap().title, "Build UI layout");

        // Filter for "database"
        picker.push_char('d');
        picker.push_char('a');
        picker.push_char('t');
        picker.push_char('a');
        assert_eq!(picker.filtered_indices.len(), 1);
        assert_eq!(picker.selected_thread().unwrap().title, "Refactor database");

        // Pop chars
        picker.pop_char();
        picker.pop_char();
        picker.pop_char();
        picker.pop_char();
        assert_eq!(picker.filtered_indices.len(), 2);
    }

    #[test]
    fn test_format_relative_time() {
        let now = chrono::Utc::now().timestamp();
        assert_eq!(format_relative_time(now - 10), "just now");
        assert_eq!(format_relative_time(now - 120), "2m ago");
        assert_eq!(format_relative_time(now - 7200), "2h ago");
        assert_eq!(format_relative_time(now - 172800), "2d ago");
    }
}
