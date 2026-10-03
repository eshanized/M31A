//! Interactive Prompt Composer Component (COP-01, CLI-01, CLI-04, D-04).
//!
//! Provides the primary operator input surface for the M31A Cockpit:
//! - Natural language prompt composition with full Unicode support
//! - Multiline editing with block quotes (`"""`) and line continuation (`\`)
//! - Centralized slash command autocomplete popup (sourced from `SlashCommandRegistry`)
//! - Real-time `@file` and `@directory` mention autocompletion
//! - Input history navigation (Up/Down traversal)
//! - Clean rendering with zero DB/filesystem I/O during draw

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::path::{Path, PathBuf};

use crate::interaction::commands::SlashCommandRegistry;
use crate::tui::input::text_input::TextInput;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Classification of autocomplete suggestions currently shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutocompleteKind {
    SlashCommand,
    Mention,
}

/// A single autocomplete suggestion item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutocompleteSuggestion {
    pub label: String,
    pub description: String,
    pub insert_text: String,
}

/// Outcome of key event processing in the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerAction {
    /// Text was submitted for execution.
    Submit(String),
    /// Cancel requested (e.g. Esc pressed when no autocomplete active).
    Cancel,
    /// Key was absorbed internally by composer.
    None,
}

/// The interactive prompt composer widget state.
pub struct TuiComposer {
    input: TextInput,
    history: Vec<String>,
    history_idx: Option<usize>,
    workspace_root: PathBuf,
    cached_files: Vec<String>,
    cached_models: Vec<(String, String)>,
    is_autocomplete_open: bool,
    autocomplete_kind: Option<AutocompleteKind>,
    autocomplete_items: Vec<AutocompleteSuggestion>,
    autocomplete_selected: usize,
    multiline: bool,
}

impl std::fmt::Debug for TuiComposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiComposer")
            .field("text", &self.input.text())
            .field("history_len", &self.history.len())
            .field("is_autocomplete_open", &self.is_autocomplete_open)
            .finish()
    }
}

impl Default for TuiComposer {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

impl TuiComposer {
    /// Create a new interactive composer anchored to a workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        let mut composer = Self {
            input: TextInput::multi_line().with_placeholder(
                "Ask M31A a question, / for commands, @ to mention files... (Enter to send)",
            ),
            history: Vec::new(),
            history_idx: None,
            workspace_root: workspace_root.clone(),
            cached_files: Vec::new(),
            cached_models: Vec::new(),
            is_autocomplete_open: false,
            autocomplete_kind: None,
            autocomplete_items: Vec::new(),
            autocomplete_selected: 0,
            multiline: false,
        };
        composer.refresh_file_cache();
        composer.refresh_model_cache();
        composer
    }

    /// Access current text buffer value.
    pub fn text(&self) -> &str {
        self.input.text()
    }

    /// Explicitly set the text buffer value.
    pub fn set_text(&mut self, text: &str) {
        self.input.set_text(text);
        self.update_autocomplete();
    }

    /// Clear text buffer and reset autocomplete.
    pub fn clear(&mut self) {
        self.input.clear();
        self.history_idx = None;
        self.multiline = false;
        self.close_autocomplete();
    }

    /// Paste text into composer buffer, expanding multiline mode if needed.
    pub fn paste(&mut self, text: &str) {
        self.input.insert_str(text);
        if text.contains('\n') {
            self.multiline = true;
        }
        self.update_autocomplete();
    }

    /// Populate cached workspace files for fast, non-blocking autocomplete.
    pub fn refresh_file_cache(&mut self) {
        let mut files = Vec::new();
        collect_workspace_files(&self.workspace_root, &self.workspace_root, &mut files, 0);
        files.sort();
        self.cached_files = files;
    }

    /// Populate cached model suggestions from .m31a/cache/model_catalog.json if available.
    pub fn refresh_model_cache(&mut self) {
        let cache_path = crate::model::catalog::ModelCatalog::cache_path_for_channel(
            &self.workspace_root,
            crate::deployment::DeploymentChannel::current(),
        );
        if let Ok(catalog) = crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path)
            && !catalog.is_empty()
        {
            self.cached_models = catalog
                .models
                .into_iter()
                .map(|c| {
                    let desc = format!(
                        "{} | Tier: {} | Ctx: {}",
                        c.provider, c.tier, c.context_capacity
                    );
                    (c.model_id, desc)
                })
                .collect();
            return;
        }
        // Authoritative fallback defaults matching NVIDIA NIM production catalog
        self.cached_models = vec![
            (
                "meta/llama-3.1-70b-instruct".to_string(),
                "NVIDIA NIM primary reasoning model".to_string(),
            ),
            (
                "meta/llama-3.2-11b-vision-instruct".to_string(),
                "NVIDIA NIM fast auxiliary model".to_string(),
            ),
            (
                "meta/llama-3.3-70b-instruct".to_string(),
                "NVIDIA NIM advanced reasoning model".to_string(),
            ),
            (
                "deepseek-ai/deepseek-r1".to_string(),
                "NVIDIA NIM deep reasoning model".to_string(),
            ),
        ];
    }

    /// Whether autocomplete popup is currently open.
    pub fn is_autocomplete_open(&self) -> bool {
        self.is_autocomplete_open
    }

    /// Close autocomplete popup.
    pub fn close_autocomplete(&mut self) {
        self.is_autocomplete_open = false;
        self.autocomplete_kind = None;
        self.autocomplete_items.clear();
        self.autocomplete_selected = 0;
    }

    /// Process a keyboard event. Returns a `ComposerAction`.
    pub fn handle_key(&mut self, key: KeyEvent) -> ComposerAction {
        // 1. If autocomplete popup is active, it takes priority
        if self.is_autocomplete_open {
            match key.code {
                KeyCode::Up => {
                    if !self.autocomplete_items.is_empty() {
                        if self.autocomplete_selected == 0 {
                            self.autocomplete_selected = self.autocomplete_items.len() - 1;
                        } else {
                            self.autocomplete_selected -= 1;
                        }
                    }
                    return ComposerAction::None;
                }
                KeyCode::Down => {
                    if !self.autocomplete_items.is_empty() {
                        self.autocomplete_selected =
                            (self.autocomplete_selected + 1) % self.autocomplete_items.len();
                    }
                    return ComposerAction::None;
                }
                KeyCode::Tab | KeyCode::Enter => {
                    self.accept_autocomplete();
                    return ComposerAction::None;
                }
                KeyCode::Esc => {
                    self.close_autocomplete();
                    return ComposerAction::None;
                }
                _ => {}
            }
        }

        // Ctrl+J inserts a newline (standard Unix terminal newline)
        if key.code == KeyCode::Char('j') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let mut new_text = self.input.text().to_string();
            new_text.push('\n');
            self.input.set_text(&new_text);
            self.multiline = true;
            return ComposerAction::None;
        }

        // 2. Standard composer navigation and submission
        match key.code {
            KeyCode::Enter => {
                let text = self.input.text();
                // Check if multiline continuation is requested
                let is_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                let is_alt = key.modifiers.contains(KeyModifiers::ALT);
                let ends_with_backslash = text.ends_with('\\');

                if is_shift || is_alt || ends_with_backslash {
                    if ends_with_backslash {
                        let mut new_text = text[..text.len() - 1].to_string();
                        new_text.push('\n');
                        self.input.set_text(&new_text);
                    } else {
                        let mut new_text = text.to_string();
                        new_text.push('\n');
                        self.input.set_text(&new_text);
                    }
                    self.multiline = true;
                    return ComposerAction::None;
                }

                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return ComposerAction::None;
                }

                let submitted = trimmed.to_string();
                self.history.push(submitted.clone());
                self.history_idx = None;
                self.clear();
                ComposerAction::Submit(submitted)
            }
            KeyCode::Up => {
                let (line, _) = self.input.current_line_and_col();
                // If on top line or history navigation is active, navigate history
                if (line == 0 && !self.multiline)
                    || self.input.text().is_empty()
                    || self.history_idx.is_some()
                {
                    self.history_prev();
                    ComposerAction::None
                } else {
                    self.input.handle_key(key);
                    self.update_autocomplete();
                    ComposerAction::None
                }
            }
            KeyCode::Down => {
                if self.history_idx.is_some() {
                    self.history_next();
                    ComposerAction::None
                } else {
                    self.input.handle_key(key);
                    self.update_autocomplete();
                    ComposerAction::None
                }
            }
            KeyCode::Esc => {
                if !self.input.text().is_empty() {
                    self.clear();
                    ComposerAction::None
                } else {
                    ComposerAction::Cancel
                }
            }
            KeyCode::Tab => {
                // Manually trigger autocomplete if not open
                self.update_autocomplete();
                if self.is_autocomplete_open && !self.autocomplete_items.is_empty() {
                    self.accept_autocomplete();
                }
                ComposerAction::None
            }
            _ => {
                let changed = self.input.handle_key(key);
                if changed {
                    self.update_autocomplete();
                }
                ComposerAction::None
            }
        }
    }

    /// Navigate backward through command history.
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_idx = match self.history_idx {
            None => self.history.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };

        self.history_idx = Some(new_idx);
        if let Some(item) = self.history.get(new_idx) {
            self.input.set_text(item);
        }
    }

    /// Navigate forward through command history.
    fn history_next(&mut self) {
        match self.history_idx {
            None => {}
            Some(i) => {
                if i + 1 < self.history.len() {
                    let next = i + 1;
                    self.history_idx = Some(next);
                    if let Some(item) = self.history.get(next) {
                        self.input.set_text(item);
                    }
                } else {
                    self.history_idx = None;
                    self.input.clear();
                }
            }
        }
    }

    /// Apply selected autocomplete suggestion to the input buffer.
    fn accept_autocomplete(&mut self) {
        if self.autocomplete_items.is_empty()
            || self.autocomplete_selected >= self.autocomplete_items.len()
        {
            self.close_autocomplete();
            return;
        }

        let suggestion = &self.autocomplete_items[self.autocomplete_selected];
        let text = self.input.text();

        match self.autocomplete_kind {
            Some(AutocompleteKind::SlashCommand) => {
                // Replaces leading slash or input with chosen command
                self.input.set_text(&format!("{} ", suggestion.insert_text));
            }
            Some(AutocompleteKind::Mention) => {
                // Replace the last typed @mention token with the selected file
                if let Some(at_idx) = text.rfind('@') {
                    let prefix = &text[..at_idx];
                    let new_text = format!("{}@{} ", prefix, suggestion.insert_text);
                    self.input.set_text(&new_text);
                }
            }
            None => {}
        }

        self.close_autocomplete();
    }

    /// Update autocomplete state based on current buffer contents.
    fn update_autocomplete(&mut self) {
        let text = self.input.text();

        // 1. Check for Slash Command prefix: "/..."
        if text.starts_with('/') && !text.contains(' ') {
            let query = &text[1..];
            let registry = SlashCommandRegistry::new_standard();
            let mut matches = Vec::new();

            for cmd in registry.commands() {
                let name = cmd.name;
                if name.starts_with(query)
                    || cmd.aliases.iter().any(|a| a.starts_with(query))
                    || query.is_empty()
                {
                    matches.push(AutocompleteSuggestion {
                        label: format!("/{}", name),
                        description: cmd.description.to_string(),
                        insert_text: format!("/{}", name),
                    });
                }
            }

            if !matches.is_empty() {
                self.autocomplete_kind = Some(AutocompleteKind::SlashCommand);
                self.autocomplete_items = matches;
                self.autocomplete_selected = 0;
                self.is_autocomplete_open = true;
                return;
            }
        }

        // 1b. Check for /model subcommand options
        if let Some(arg_prefix) = text.strip_prefix("/model ") {
            let query = arg_prefix.to_lowercase();
            let mut matches = Vec::new();
            for (m, desc) in &self.cached_models {
                if m.to_lowercase().contains(&query) || query.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: m.clone(),
                        description: desc.clone(),
                        insert_text: format!("/model {}", m),
                    });
                }
            }
            if !matches.is_empty() {
                self.autocomplete_kind = Some(AutocompleteKind::SlashCommand);
                self.autocomplete_items = matches;
                self.autocomplete_selected = 0;
                self.is_autocomplete_open = true;
                return;
            }
        }

        // 1c. Check for /profile subcommand options
        if let Some(arg_prefix) = text.strip_prefix("/profile ") {
            let profiles = [
                ("autonomous", "Full autonomy within safety constraints"),
                ("assisted", "Interactive approval for mutations"),
                ("safe", "Strict approval for external tool actions"),
                ("plan", "Read-only planning without side effects"),
                (
                    "unattended",
                    "Autonomous batch execution with budget limits",
                ),
            ];
            let query = arg_prefix.to_lowercase();
            let mut matches = Vec::new();
            for (p, desc) in profiles {
                if p.starts_with(&query) || query.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: p.to_string(),
                        description: desc.to_string(),
                        insert_text: format!("/profile {}", p),
                    });
                }
            }
            if !matches.is_empty() {
                self.autocomplete_kind = Some(AutocompleteKind::SlashCommand);
                self.autocomplete_items = matches;
                self.autocomplete_selected = 0;
                self.is_autocomplete_open = true;
                return;
            }
        }

        // 2. Check for @mention token: "... @src/..."
        if let Some(at_idx) = text.rfind('@') {
            let suffix = &text[at_idx + 1..];
            // Only trigger if no space after '@'
            if !suffix.contains(' ') {
                let query = suffix.to_lowercase();
                let mut matches = Vec::new();

                for file in &self.cached_files {
                    if file.to_lowercase().contains(&query) || query.is_empty() {
                        matches.push(AutocompleteSuggestion {
                            label: format!("@{}", file),
                            description: file.clone(),
                            insert_text: file.clone(),
                        });
                        if matches.len() >= 8 {
                            break;
                        }
                    }
                }

                if !matches.is_empty() {
                    self.autocomplete_kind = Some(AutocompleteKind::Mention);
                    self.autocomplete_items = matches;
                    self.autocomplete_selected = 0;
                    self.is_autocomplete_open = true;
                    return;
                }
            }
        }

        self.close_autocomplete();
    }

    /// Render composer and any active autocomplete popup.
    pub fn render(&self, f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
        let is_mono = tokens.mode == ThemeMode::MonochromeANSI || std::env::var("NO_COLOR").is_ok();

        // Main input block
        let block_title = if self.is_autocomplete_open {
            " Composer [Autocomplete Active: Tab/Enter to accept, Esc to cancel] "
        } else {
            " Composer [Enter: Submit | Shift+Enter/\\: Multiline | /: Commands | @: Files] "
        };

        let border_color = if is_mono {
            Color::White
        } else if self.is_autocomplete_open {
            Color::Yellow
        } else {
            Color::Cyan
        };

        let composer_block = Block::default()
            .title(block_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner_area = composer_block.inner(area);
        f.render_widget(composer_block, area);

        // Content layout: prompt prefix "m31a> " + text
        let prompt_prefix = "m31a> ";
        let full_content = if self.input.text().is_empty() {
            format!("{}Ask M31A a task or command...", prompt_prefix)
        } else {
            format!("{}{}_", prompt_prefix, self.input.text())
        };

        let content_style = if self.input.text().is_empty() {
            if is_mono {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            }
        } else {
            Style::default().fg(Color::White)
        };

        let p = Paragraph::new(full_content)
            .style(content_style)
            .wrap(Wrap { trim: false });
        f.render_widget(p, inner_area);

        // Position terminal cursor at active typing insertion point
        let (line_idx, col_idx) = self.input.current_line_and_col();
        let cursor_x = inner_area.x + (prompt_prefix.len() as u16) + (col_idx as u16);
        let cursor_y = inner_area.y + (line_idx as u16);
        if cursor_x < inner_area.right() && cursor_y < inner_area.bottom() {
            f.set_cursor_position((cursor_x, cursor_y));
        }

        // Floating Autocomplete Popup
        if self.is_autocomplete_open && !self.autocomplete_items.is_empty() {
            let popup_height = (self.autocomplete_items.len() as u16 + 2).min(10);
            let popup_width = 50.min(area.width.saturating_sub(4));
            let popup_y = area.y.saturating_sub(popup_height);
            let popup_area = Rect {
                x: area.x + 2,
                y: popup_y,
                width: popup_width,
                height: popup_height,
            };

            f.render_widget(Clear, popup_area);

            let title = match self.autocomplete_kind {
                Some(AutocompleteKind::SlashCommand) => " Slash Commands ",
                Some(AutocompleteKind::Mention) => " Files & Directories ",
                None => " Suggestions ",
            };

            let popup_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(if is_mono {
                    Style::default()
                } else {
                    Style::default().fg(Color::Yellow)
                });

            let mut lines = Vec::new();
            for (idx, item) in self.autocomplete_items.iter().enumerate() {
                let is_sel = idx == self.autocomplete_selected;
                let prefix = if is_sel { "> " } else { "  " };
                let style = if is_sel {
                    if is_mono {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(Color::Gray)
                };

                let line_str = format!(
                    "{:<18} {}",
                    format!("{}{}", prefix, item.label),
                    item.description
                );
                lines.push(ratatui::text::Line::styled(line_str, style));
            }

            let popup_paragraph = Paragraph::new(lines).block(popup_block);
            f.render_widget(popup_paragraph, popup_area);
        }
    }
}

/// Recursively collect relative file paths from the workspace up to depth 4,
/// ignoring common build directories and hidden folders.
fn collect_workspace_files(
    root: &Path,
    current_dir: &Path,
    output: &mut Vec<String>,
    depth: usize,
) {
    if depth > 4 || output.len() >= 300 {
        return;
    }

    let entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Skip hidden files, target, git, node_modules
        if name_str.starts_with('.')
            || name_str == "target"
            || name_str == "node_modules"
            || name_str == "dist"
        {
            continue;
        }

        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            if path.is_file() {
                output.push(rel_str);
            } else if path.is_dir() {
                output.push(format!("{}/", rel_str));
                collect_workspace_files(root, &path, output, depth + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composer_basic_input_and_submit() {
        let mut composer = TuiComposer::new(PathBuf::from("."));
        assert_eq!(composer.text(), "");

        composer.set_text("Hello M31A");
        assert_eq!(composer.text(), "Hello M31A");

        let action = composer.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, ComposerAction::Submit("Hello M31A".to_string()));
        assert_eq!(composer.text(), "");
    }

    #[test]
    fn test_composer_slash_autocomplete_trigger() {
        let mut composer = TuiComposer::new(PathBuf::from("."));
        composer.set_text("/sta");
        assert!(composer.is_autocomplete_open());
        assert_eq!(
            composer.autocomplete_kind,
            Some(AutocompleteKind::SlashCommand)
        );

        // Tab should accept /status
        let action = composer.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(action, ComposerAction::None);
        assert!(composer.text().starts_with("/status"));
        assert!(!composer.is_autocomplete_open());
    }

    #[test]
    fn test_composer_history_navigation() {
        let mut composer = TuiComposer::new(PathBuf::from("."));
        composer.set_text("first task");
        let _ = composer.handle_key(KeyEvent::from(KeyCode::Enter));
        composer.set_text("second task");
        let _ = composer.handle_key(KeyEvent::from(KeyCode::Enter));

        // Up arrow should bring back "second task"
        let _ = composer.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(composer.text(), "second task");

        // Another Up arrow should bring back "first task"
        let _ = composer.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(composer.text(), "first task");
    }
}
