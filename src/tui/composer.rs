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
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::path::{Path, PathBuf};

use crate::interaction::commands::SlashCommandRegistry;
use crate::tui::input::text_input::TextInput;
use crate::tui::theme::ThemeTokens;

/// Fallback label width (chars) for the autocomplete popup when there are
/// no suggestions. ALGORITHMIC/layout constant: pure rendering geometry,
/// never configuration or runtime policy.
pub const COMPOSER_EMPTY_LABEL_WIDTH: usize = 8;

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
///
/// Autocomplete is a small explicit state machine:
/// - `is_autocomplete_open == true` implies `!autocomplete_items.is_empty()`.
/// - `autocomplete_selected` is always `< items.len()` while open.
/// - `autocomplete_user_navigated` tracks explicit Up/Down movement so
///   Enter can distinguish "accept completion" from "submit exact command".
/// - `autocomplete_scroll` keeps the selected row inside the visible window.
pub struct TuiComposer {
    input: TextInput,
    history: Vec<String>,
    history_idx: Option<usize>,
    workspace_root: PathBuf,
    cached_files: Vec<String>,
    cached_models: Vec<(String, String)>,
    slash_registry: Option<std::sync::Arc<SlashCommandRegistry>>,
    snapshot_handle: Option<crate::interaction::user_commands::CommandSnapshotHandle>,
    is_autocomplete_open: bool,
    autocomplete_kind: Option<AutocompleteKind>,
    autocomplete_items: Vec<AutocompleteSuggestion>,
    autocomplete_selected: usize,
    autocomplete_user_navigated: bool,
    autocomplete_scroll: usize,
    multiline: bool,
    file_cache_loaded: bool,
    autocomplete_generation: u64,
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
    /// Does not perform blocking filesystem scans or catalog loads during construction (0-I/O).
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            input: TextInput::multi_line().with_placeholder(
                "Ask M31A a question, / for commands, @ to mention files... (Enter to send)",
            ),
            history: Vec::new(),
            history_idx: None,
            workspace_root,
            cached_files: Vec::new(),
            cached_models: Vec::new(),
            slash_registry: None,
            snapshot_handle: None,
            is_autocomplete_open: false,
            autocomplete_kind: None,
            autocomplete_items: Vec::new(),
            autocomplete_selected: 0,
            autocomplete_user_navigated: false,
            autocomplete_scroll: 0,
            multiline: false,
            file_cache_loaded: false,
            autocomplete_generation: 0,
        }
    }

    /// Builder to initialize composer with pre-cached models without I/O.
    pub fn with_cached_models(mut self, models: Vec<(String, String)>) -> Self {
        self.cached_models = models;
        self
    }

    /// Update the workspace root, invalidating cached files and models if the path changes (AUD-26).
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        if self.workspace_root != root {
            self.workspace_root = root;
            self.file_cache_loaded = false;
            self.cached_files.clear();
            self.cached_models.clear();
            self.autocomplete_generation = self.autocomplete_generation.wrapping_add(1);
            if self.is_autocomplete_open {
                self.update_autocomplete();
            }
        }
    }

    /// Set an explicit slash command registry for dynamic autocompletion.
    pub fn with_slash_registry(mut self, registry: std::sync::Arc<SlashCommandRegistry>) -> Self {
        self.slash_registry = Some(registry);
        self.snapshot_handle = None;
        self
    }

    /// Set an explicit slash command registry for dynamic autocompletion.
    pub fn set_slash_registry(&mut self, registry: std::sync::Arc<SlashCommandRegistry>) {
        self.slash_registry = Some(registry);
        self.snapshot_handle = None;
        if self.is_autocomplete_open {
            self.update_autocomplete();
        }
    }

    /// Set a command snapshot handle for live dynamic autocompletion.
    pub fn with_snapshot_handle(
        mut self,
        handle: crate::interaction::user_commands::CommandSnapshotHandle,
    ) -> Self {
        self.slash_registry = Some(handle.current_registry());
        self.snapshot_handle = Some(handle);
        self
    }

    /// Set a command snapshot handle for live dynamic autocompletion.
    pub fn set_snapshot_handle(
        &mut self,
        handle: crate::interaction::user_commands::CommandSnapshotHandle,
    ) {
        self.slash_registry = Some(handle.current_registry());
        self.snapshot_handle = Some(handle);
        if self.is_autocomplete_open {
            self.update_autocomplete();
        }
    }

    /// Access the bound slash command registry if set.
    pub fn slash_registry(&self) -> Option<std::sync::Arc<SlashCommandRegistry>> {
        if let Some(ref h) = self.snapshot_handle {
            Some(h.current_registry())
        } else {
            self.slash_registry.clone()
        }
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

    /// Ensure cached workspace files are populated lazily on demand.
    pub fn ensure_file_cache(&mut self) {
        if !self.file_cache_loaded {
            self.refresh_file_cache();
        }
    }

    /// Populate cached workspace files for fast, non-blocking autocomplete.
    pub fn refresh_file_cache(&mut self) {
        let mut files = Vec::new();
        let mut budget = MAX_FS_VISITED_ENTRIES;
        collect_workspace_files(
            &self.workspace_root,
            &self.workspace_root,
            &mut files,
            0,
            &mut budget,
        );
        files.sort();
        self.cached_files = files;
        self.file_cache_loaded = true;
    }

    /// Explicitly invalidate autocomplete caches when catalog, commands, or files change.
    pub fn invalidate_cache(&mut self) {
        self.file_cache_loaded = false;
        self.cached_files.clear();
        self.refresh_model_cache();
        self.autocomplete_generation = self.autocomplete_generation.wrapping_add(1);
        if self.is_autocomplete_open {
            self.update_autocomplete();
        }
    }

    /// Monotonic generation token for autocomplete state to prevent async race conditions.
    pub fn autocomplete_generation(&self) -> u64 {
        self.autocomplete_generation
    }

    /// Apply asynchronously resolved autocomplete suggestions if the generation token matches.
    pub fn set_async_autocomplete_results(
        &mut self,
        generation: u64,
        kind: AutocompleteKind,
        items: Vec<AutocompleteSuggestion>,
    ) -> bool {
        if generation != self.autocomplete_generation {
            return false;
        }
        self.open_autocomplete(kind, items);
        true
    }

    /// Populate cached model suggestions from the canonical catalog authority
    /// (global platform cache, legacy workspace cache as migration fallback).
    pub fn refresh_model_cache(&mut self) {
        if let Some(catalog) =
            crate::model::catalog::ModelCatalog::load_canonical_for_workspace(&self.workspace_root)
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
        // When catalog is missing or empty, do not fabricate fallback models.
        self.cached_models = Vec::new();
    }

    /// Access currently cached model suggestions.
    pub fn cached_models(&self) -> &[(String, String)] {
        &self.cached_models
    }

    /// Set cached model suggestions for testing or programmatic override.
    pub fn set_cached_models(&mut self, models: Vec<(String, String)>) {
        self.cached_models = models;
    }

    /// Update cached models from an authoritative catalog source and invalidate autocomplete suggestions (M31A-UI-11).
    pub fn refresh_from_catalog(&mut self, models: Vec<(String, String)>) {
        self.cached_models = models;
        self.autocomplete_generation = self.autocomplete_generation.wrapping_add(1);
        if self.is_autocomplete_open {
            self.update_autocomplete();
        }
    }

    /// Whether autocomplete popup is currently open.
    pub fn is_autocomplete_open(&self) -> bool {
        self.is_autocomplete_open
    }

    /// Autocomplete suggestions currently available.
    pub fn autocomplete_suggestions(&self) -> &[AutocompleteSuggestion] {
        &self.autocomplete_items
    }

    /// Whether the workspace file cache has been loaded into memory.
    pub fn is_file_cache_loaded(&self) -> bool {
        self.file_cache_loaded
    }

    /// Close autocomplete popup.
    pub fn close_autocomplete(&mut self) {
        self.is_autocomplete_open = false;
        self.autocomplete_kind = None;
        self.autocomplete_items.clear();
        self.autocomplete_selected = 0;
        self.autocomplete_user_navigated = false;
        self.autocomplete_scroll = 0;
    }

    /// Open autocomplete with a validated non-empty item list.
    ///
    /// Central invariant: the popup is never "open but empty".
    pub fn open_autocomplete(
        &mut self,
        kind: AutocompleteKind,
        items: Vec<AutocompleteSuggestion>,
    ) {
        if items.is_empty() {
            self.close_autocomplete();
            return;
        }
        let prev_selected_key = if self.autocomplete_user_navigated {
            self.autocomplete_items
                .get(self.autocomplete_selected)
                .map(|s| s.insert_text.clone())
        } else {
            None
        };
        self.autocomplete_kind = Some(kind);
        self.autocomplete_items = items;
        if let Some(prev) = prev_selected_key {
            if let Some(pos) = self
                .autocomplete_items
                .iter()
                .position(|m| m.insert_text == prev)
            {
                self.autocomplete_selected = pos;
            } else {
                self.autocomplete_selected = 0;
                self.autocomplete_user_navigated = false;
            }
        } else if self.autocomplete_selected >= self.autocomplete_items.len() {
            self.autocomplete_selected = 0;
            self.autocomplete_scroll = 0;
        }
        // `autocomplete_user_navigated` is reset by the caller
        // (`update_autocomplete`) on every fresh query; it is preserved
        // across Up/Down navigation.
        self.is_autocomplete_open = true;
        self.clamp_selection_visible(usize::MAX);
    }

    /// Autocomplete classification currently shown.
    pub fn autocomplete_kind(&self) -> Option<AutocompleteKind> {
        self.autocomplete_kind
    }

    /// Currently selected suggestion index.
    pub fn autocomplete_selected_index(&self) -> usize {
        self.autocomplete_selected
    }

    /// Currently selected suggestion, if the popup is open.
    pub fn selected_suggestion(&self) -> Option<&AutocompleteSuggestion> {
        if self.is_autocomplete_open {
            self.autocomplete_items.get(self.autocomplete_selected)
        } else {
            None
        }
    }

    /// Current scroll offset for the suggestion window.
    pub fn autocomplete_scroll_offset(&self) -> usize {
        self.autocomplete_scroll
    }

    /// Whether the user explicitly moved selection with Up/Down since the
    /// popup opened. Used by the Enter state machine (Case D).
    pub fn autocomplete_user_navigated(&self) -> bool {
        self.autocomplete_user_navigated
    }

    /// Resolve the registry authority: explicit binding, snapshot handle, or a standard snapshot.
    fn registry_snapshot(&self) -> (std::sync::Arc<SlashCommandRegistry>, bool) {
        if let Some(ref handle) = self.snapshot_handle {
            return (handle.current_registry(), false);
        }
        match &self.slash_registry {
            Some(r) => (r.clone(), false),
            None => (
                std::sync::Arc::new(SlashCommandRegistry::new_standard()),
                true,
            ),
        }
    }

    /// True when `text` is already an exact valid command name
    /// (`/help`, `/st`, `/?`, …) with no arguments attached.
    pub fn is_exact_command(&self, text: &str) -> bool {
        let trimmed = text.trim();
        if !trimmed.starts_with('/') || trimmed.contains(char::is_whitespace) {
            return false;
        }
        let needle = trimmed.trim_start_matches('/').to_lowercase();
        if needle.is_empty() {
            return false;
        }
        let (reg, _owned) = self.registry_snapshot();
        match reg.find(&needle) {
            Some(cmd) => {
                cmd.name.eq_ignore_ascii_case(&needle)
                    || cmd.aliases.iter().any(|a| a.eq_ignore_ascii_case(&needle))
            }
            None => false,
        }
    }

    /// Enter state machine (Cases A–D):
    /// - exact command (`/help`) → submit, never force a second Enter;
    /// - partial (`/hel`) → accept completion, do NOT submit yet;
    /// - explicit Up/Down navigation away from exact input → accept;
    /// - command with args (`/help foo`) → submit normally.
    pub fn should_submit_on_enter(&self) -> bool {
        if !self.is_autocomplete_open {
            return true;
        }
        let text = self.input.text().trim().to_string();
        // Case C: arguments attached — command-name completion is no longer
        // relevant. Arg-value popups (`/model <x>`) still want Enter=accept;
        // bare-command popups with args want Enter=submit.
        if text.contains(char::is_whitespace) {
            return match self.autocomplete_kind {
                Some(AutocompleteKind::SlashCommand) => {
                    // Bare-command popup only opens for single-token input
                    // (see update_autocomplete). If we somehow still show it
                    // with args present, submit. Arg-value popups accept.
                    !self.is_bare_command_popup()
                }
                Some(AutocompleteKind::Mention) => false,
                None => true,
            };
        }
        // Case B: exact valid command submits with a single Enter.
        if self.is_exact_command(&text) && !self.autocomplete_user_navigated {
            return true;
        }
        // Case D: explicit navigation → accept.
        if self.autocomplete_user_navigated {
            // If navigation landed back on the exact input, still submit.
            if let Some(sel) = self.selected_suggestion()
                && sel.insert_text.trim() == text
                && self.is_exact_command(&text)
            {
                return true;
            }
            return false;
        }
        // No navigation: accept when completion would change the input
        // (selected differs), submit when the input already equals the
        // selected suggestion (nothing left to complete).
        match self.selected_suggestion() {
            Some(sel) => sel.insert_text.trim() == text,
            None => true,
        }
    }

    /// True when the open popup lists bare slash-command names
    /// (as opposed to `/model <value>` / `/profile <value>` arg values).
    fn is_bare_command_popup(&self) -> bool {
        match self.autocomplete_kind {
            Some(AutocompleteKind::SlashCommand) => {
                // Arg-value suggestions carry a space inside insert_text
                // (`/model <id>`); bare-command suggestions never do.
                self.autocomplete_items
                    .first()
                    .is_none_or(|s| !s.insert_text.contains(' '))
            }
            _ => false,
        }
    }

    /// Keep `autocomplete_selected` inside the visible window.
    fn clamp_selection_visible(&mut self, max_visible: usize) {
        if self.autocomplete_items.is_empty() {
            self.autocomplete_scroll = 0;
            return;
        }
        if self.autocomplete_selected >= self.autocomplete_items.len() {
            self.autocomplete_selected = self.autocomplete_items.len() - 1;
        }
        if max_visible == usize::MAX || max_visible == 0 {
            // Unknown viewport (state update path): keep scroll minimal.
            if self.autocomplete_selected < self.autocomplete_scroll {
                self.autocomplete_scroll = self.autocomplete_selected;
            }
            return;
        }
        if self.autocomplete_selected < self.autocomplete_scroll {
            self.autocomplete_scroll = self.autocomplete_selected;
        } else if self.autocomplete_selected >= self.autocomplete_scroll + max_visible {
            self.autocomplete_scroll = self.autocomplete_selected + 1 - max_visible;
        }
    }

    /// Process a keyboard event. Returns a `ComposerAction`.
    pub fn handle_key(&mut self, key: KeyEvent) -> ComposerAction {
        // 1. Autocomplete-aware key routing (explicit state machine).
        if self.is_autocomplete_open {
            match key.code {
                KeyCode::Up => {
                    if !self.autocomplete_items.is_empty() {
                        if self.autocomplete_selected == 0 {
                            self.autocomplete_selected = self.autocomplete_items.len() - 1;
                        } else {
                            self.autocomplete_selected -= 1;
                        }
                        self.autocomplete_user_navigated = true;
                        // Visible window is viewport-dependent; keep scroll
                        // consistent with a generous default here and let the
                        // renderer clamp precisely per frame.
                        self.clamp_selection_visible(8);
                    }
                    return ComposerAction::None;
                }
                KeyCode::Down => {
                    if !self.autocomplete_items.is_empty() {
                        self.autocomplete_selected =
                            (self.autocomplete_selected + 1) % self.autocomplete_items.len();
                        self.autocomplete_user_navigated = true;
                        self.clamp_selection_visible(8);
                    }
                    return ComposerAction::None;
                }
                KeyCode::Tab => {
                    // Tab always accepts the selected suggestion.
                    self.accept_autocomplete();
                    return ComposerAction::None;
                }
                KeyCode::Enter => {
                    // Terminal/editor semantics (Cases A–D):
                    // exact command → submit; partial → accept only.
                    if self.should_submit_on_enter() {
                        self.close_autocomplete();
                        // Fall through to the standard submission path below.
                    } else {
                        self.accept_autocomplete();
                        return ComposerAction::None;
                    }
                }
                KeyCode::Esc => {
                    // Case E: dismiss popup, preserve the user's text.
                    self.close_autocomplete();
                    return ComposerAction::None;
                }
                _ => {}
            }
        }

        // Ctrl+J inserts a newline (standard Unix terminal newline) at caret position
        if key.code == KeyCode::Char('j') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.input
                .set_mode(crate::tui::input::text_input::InputMode::MultiLine);
            self.input.insert_char('\n');
            self.multiline = true;
            self.update_autocomplete();
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
                    self.input
                        .set_mode(crate::tui::input::text_input::InputMode::MultiLine);
                    if ends_with_backslash
                        && self.input.cursor_char_index() == self.input.char_count()
                    {
                        self.input.delete_backward();
                    }
                    self.input.insert_char('\n');
                    self.multiline = true;
                    self.update_autocomplete();
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
    ///
    /// Context-aware whitespace: bare slash completions never gain a
    /// trailing space (`/hel` → `/help`, not `/help `). Arg-value
    /// completions (`/model <id>`) expand to the full command line with the
    /// cursor left at the end. `@mention` replacement touches only the
    /// current mention token and preserves all surrounding text.
    fn accept_autocomplete(&mut self) {
        if self.autocomplete_items.is_empty()
            || self.autocomplete_selected >= self.autocomplete_items.len()
        {
            self.close_autocomplete();
            return;
        }

        let suggestion = self.autocomplete_items[self.autocomplete_selected].clone();
        let text = self.input.text().to_string();

        match self.autocomplete_kind {
            Some(AutocompleteKind::SlashCommand) => {
                // insert_text is already the full replacement line
                // (`/help` or `/model <id>`); never append whitespace.
                let completed = suggestion.insert_text.trim_end().to_string();
                self.input.set_text(&completed);
            }
            Some(AutocompleteKind::Mention) => {
                // Replace only the current mention token.
                if let Some(at_idx) = text.rfind('@') {
                    let prefix = &text[at_idx..];
                    // Only replace when the suffix is a live token (no space).
                    if !prefix.contains(' ') && !prefix.contains('\n') {
                        let head = &text[..at_idx];
                        let completed = suggestion.insert_text.trim_end();
                        let new_text = format!("{head}@{completed}");
                        self.input.set_text(&new_text);
                    }
                }
            }
            None => {}
        }

        // Completion is terminal for this popup lifetime: close without
        // auto-reopening so an exact command (`/help`) never traps the user
        // in a single-item popup. Typing continues to drive fresh popups via
        // `update_autocomplete` on the next keystroke.
        self.close_autocomplete();
    }

    /// Accept the selected suggestion explicitly (Tab path + tests).
    pub fn accept_selected(&mut self) {
        self.accept_autocomplete();
    }

    /// Update autocomplete state based on current buffer contents.
    ///
    /// Every path funnels through `open_autocomplete` / `close_autocomplete`
    /// so the popup is never "open but empty".
    fn update_autocomplete(&mut self) {
        let text = self.input.text().to_string();
        self.autocomplete_generation = self.autocomplete_generation.wrapping_add(1);
        // Fresh query invalidates prior explicit navigation.
        self.autocomplete_user_navigated = false;
        self.autocomplete_scroll = 0;
        self.autocomplete_selected = 0;

        // 1. Bare slash-command names: single token starting with '/'.
        if text.starts_with('/') && !text.contains(char::is_whitespace) {
            let query = text[1..].to_lowercase();
            let (reg, _owned) = self.registry_snapshot();
            let mut matches = Vec::new();

            for cmd in reg.commands() {
                if cmd.name.to_lowercase().starts_with(&query) || query.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: format!("/{}", cmd.name),
                        description: cmd.description.clone(),
                        insert_text: format!("/{}", cmd.name),
                    });
                }
                for alias in &cmd.aliases {
                    if alias.to_lowercase().starts_with(&query) || query.is_empty() {
                        matches.push(AutocompleteSuggestion {
                            label: format!("/{}", alias),
                            description: format!("{} (alias of /{})", cmd.description, cmd.name),
                            insert_text: format!("/{}", alias),
                        });
                    }
                }
            }
            matches.sort_by(|a, b| a.label.cmp(&b.label));

            if !matches.is_empty() {
                self.open_autocomplete(AutocompleteKind::SlashCommand, matches);
                return;
            }
        }

        // 1b. `/model <prefix>` arg-value completion (registry-independent
        // model catalog; the command name itself still comes from the
        // SlashCommandRegistry authority).
        if let Some(arg_prefix) = text.strip_prefix("/model ") {
            let query = arg_prefix.to_lowercase();
            let models = &self.cached_models;
            if models.is_empty() {
                // Projection-only: render an explicit empty state when models cannot be obtained.
                // Never fabricate a synthetic model inventory.
                self.open_autocomplete(
                    AutocompleteKind::SlashCommand,
                    vec![AutocompleteSuggestion {
                        label: "(no models available)".to_string(),
                        description: "Model catalog empty or loading; run /model to check status"
                            .to_string(),
                        insert_text: "/model ".to_string(),
                    }],
                );
                return;
            }
            let mut matches = Vec::new();
            for (m, desc) in models {
                if m.to_lowercase().contains(&query) || query.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: m.clone(),
                        description: desc.clone(),
                        insert_text: format!("/model {m}"),
                    });
                }
            }
            if !matches.is_empty() {
                self.open_autocomplete(AutocompleteKind::SlashCommand, matches);
                return;
            }
        }

        // 1c. `/profile <prefix>` arg-value completion (canonical universe).
        if let Some(arg_prefix) = text.strip_prefix("/profile ") {
            let profiles = [
                ("balanced", "Standard workflow with review checkpoints"),
                ("autonomous", "Higher autonomy for trusted repositories"),
                ("conservative", "Strict approval for every modification"),
                ("code_reviewer", "Read-only inspection and critique"),
                ("safe", "Minimal latitude, approvals required"),
                ("coding", "Assisted coding with writes and tests"),
                ("research", "Read-only research and discovery"),
                ("ci", "Unattended CI execution"),
                ("security_review", "Read-only security audit"),
                ("release", "Assisted release packaging"),
            ];
            let query = arg_prefix.to_lowercase();
            let mut matches = Vec::new();
            for (p, desc) in profiles {
                if p.starts_with(&query) || query.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: p.to_string(),
                        description: desc.to_string(),
                        insert_text: format!("/profile {p}"),
                    });
                }
            }
            if !matches.is_empty() {
                self.open_autocomplete(AutocompleteKind::SlashCommand, matches);
                return;
            }
        }

        // 2. `@mention` token: only the live token after the last '@'.
        if let Some(at_idx) = text.rfind('@')
            && !text[at_idx + 1..].contains(' ')
            && !text[at_idx + 1..].contains('\n')
        {
            self.ensure_file_cache();
            let suffix = text[at_idx + 1..].to_lowercase();
            let mut matches = Vec::new();

            for file in &self.cached_files.clone() {
                if file.to_lowercase().contains(&suffix) || suffix.is_empty() {
                    matches.push(AutocompleteSuggestion {
                        label: format!("@{file}"),
                        description: truncate_chars(file, 48),
                        insert_text: file.clone(),
                    });
                    if matches.len() >= 8 {
                        break;
                    }
                }
            }

            if !matches.is_empty() {
                self.open_autocomplete(AutocompleteKind::Mention, matches);
                return;
            }
        }

        self.close_autocomplete();
    }

    // ── Geometry helpers (deterministic, viewport-bounded) ──────────────

    /// Horizontal margins kept around the popup.
    pub const POPUP_MARGIN: u16 = 2;
    /// Maximum suggestion rows visible without scrolling.
    pub const POPUP_MAX_ROWS: usize = 8;

    /// Width available for the popup inside `area` (margins reserved).
    pub fn available_width(area: Rect) -> u16 {
        area.width.saturating_sub(Self::POPUP_MARGIN * 2).max(10)
    }

    /// Height available above the composer for the popup.
    pub fn available_height_above(composer_area: Rect) -> u16 {
        composer_area.y
    }

    /// Number of suggestion rows that fit in `max_height` popup budget.
    pub fn visible_suggestion_count(&self, max_height: u16) -> usize {
        let budget_rows = max_height.saturating_sub(2) as usize;
        budget_rows
            .min(Self::POPUP_MAX_ROWS)
            .min(self.autocomplete_items.len())
            .max(if self.autocomplete_items.is_empty() {
                0
            } else {
                1
            })
    }

    /// Compute the popup rectangle anchored to the composer area.
    ///
    /// - width derives from terminal dims (never hardcoded 50);
    /// - never exceeds terminal bounds; horizontal margins kept;
    /// - prefers above the composer; clamps safely when room is short;
    /// - height reflects the actual visible row count.
    pub fn popup_rect(&self, terminal: Rect, composer_area: Rect) -> Option<Rect> {
        if !self.is_autocomplete_open || self.autocomplete_items.is_empty() {
            return None;
        }
        let avail_w = Self::available_width(composer_area).min(Self::available_width(terminal));
        if avail_w < 20 {
            return None;
        }
        // Required width from content, bounded by viewport.
        let max_label = self
            .autocomplete_items
            .iter()
            .map(|s| s.label.chars().count())
            .max()
            .unwrap_or(COMPOSER_EMPTY_LABEL_WIDTH);
        let label_col = max_label
            .clamp(8, 24)
            .min((avail_w as usize).saturating_sub(10) / 2 + 8);
        let want = (label_col + 6 + 24).min(avail_w as usize);
        let popup_width = (want as u16)
            .clamp(24, avail_w)
            .min(terminal.width.saturating_sub(2));
        if popup_width < 20 {
            return None;
        }
        let room_above = composer_area.y.saturating_sub(terminal.y);
        let room_below = terminal.bottom().saturating_sub(composer_area.bottom());
        let want_rows = self.autocomplete_items.len().min(Self::POPUP_MAX_ROWS) as u16;
        let want_height = want_rows + 2;
        // Prefer above; use below only when above is too small and below fits.
        let (popup_y, popup_height) = if room_above >= want_height || room_above >= room_below {
            let h = want_height.min(room_above.max(3));
            (composer_area.y.saturating_sub(h), h)
        } else {
            let h = want_height.min(room_below.max(3)).min(
                terminal
                    .bottom()
                    .saturating_sub(composer_area.bottom())
                    .max(3),
            );
            (
                composer_area
                    .bottom()
                    .min(terminal.bottom().saturating_sub(h)),
                h,
            )
        };
        if popup_height < 3 {
            return None;
        }
        let mut x = composer_area.x + Self::POPUP_MARGIN;
        if x + popup_width > terminal.x + terminal.width {
            x = (terminal.x + terminal.width).saturating_sub(popup_width + 1);
        }
        let mut y = popup_y;
        if y + popup_height > terminal.y + terminal.height {
            y = (terminal.y + terminal.height).saturating_sub(popup_height);
        }
        Some(Rect::new(x, y, popup_width, popup_height))
    }

    /// `(label_width, desc_width)` column split for `popup_width`.
    pub fn suggestion_columns(popup_width: u16, max_label_chars: usize) -> (usize, usize) {
        let inner = (popup_width as usize).saturating_sub(2);
        if inner < 12 {
            return (inner / 2, inner / 2);
        }
        let label = max_label_chars
            .clamp(8, 24)
            .min(inner * 40 / 100 + 8)
            .min(inner - 4);
        let desc = inner.saturating_sub(label + 3);
        (label, desc)
    }

    /// Render composer and any active autocomplete popup.
    ///
    /// Editor-prompt presentation: an open `> ` line with a quiet
    /// placeholder, no boxed panel, no title noise. Extended instructions
    /// live in the footer/help — not in the prompt itself.
    pub fn render(&self, f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
        // Separator hairline on top, then open prompt area.
        let inner_area = if area.height >= 2 {
            let sep = Paragraph::new(ratatui::text::Line::from(Span::styled(
                format!(" {}", "─".repeat((area.width as usize).saturating_sub(2))),
                tokens.separator,
            )));
            f.render_widget(
                sep,
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
            );
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height.saturating_sub(1),
            }
        } else {
            area
        };
        // Content layout: prompt prefix "> " + text
        let prompt_prefix = "> ";
        let text = self.input.text();
        let is_empty = text.is_empty();
        let placeholder = "Ask M31A to build, inspect, fix, or explain...";

        if is_empty {
            // Placeholder presentation: muted text with NO active cursor
            // overlapping it. The cursor is hidden while the field is empty
            // so it can never sit on top of the placeholder glyph.
            let p = Paragraph::new(vec![ratatui::text::Line::from(vec![
                Span::styled(prompt_prefix, tokens.text_muted),
                Span::styled(placeholder, tokens.text_muted),
            ])])
            .wrap(Wrap { trim: false });
            f.render_widget(p, inner_area);
        } else {
            let p = Paragraph::new(vec![ratatui::text::Line::from(vec![
                Span::styled(prompt_prefix, tokens.text_muted),
                Span::styled(text.to_string(), tokens.text_primary),
            ])])
            .wrap(Wrap { trim: false });
            f.render_widget(p, inner_area);

            // Real insertion-point cursor with accurate unicode width and line wrapping.
            let (v_row, v_col) = calculate_visual_cursor(
                text,
                self.input.cursor_char_index(),
                prompt_prefix.chars().count() as u16,
                inner_area.width,
            );
            let cursor_x = inner_area.x.saturating_add(v_col);
            let cursor_y = inner_area.y.saturating_add(v_row);
            if cursor_x < inner_area.right() && cursor_y < inner_area.bottom() {
                f.set_cursor_position((cursor_x, cursor_y));
            }
        }

        self.render_autocomplete(f, f.area(), area, tokens);
    }

    /// Render composer without the top hairline separator (e.g. inside a bordered box).
    pub fn render_bare(&self, f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
        let prompt_prefix = "> ";
        let text = self.input.text();
        let is_empty = text.is_empty();
        let placeholder = "Ask M31A anything…";

        if is_empty {
            let p = Paragraph::new(vec![ratatui::text::Line::from(vec![
                Span::styled(prompt_prefix, tokens.text_muted),
                Span::styled(placeholder, tokens.text_muted),
            ])])
            .wrap(Wrap { trim: false });
            f.render_widget(p, area);
        } else {
            let p = Paragraph::new(vec![ratatui::text::Line::from(vec![
                Span::styled(prompt_prefix, tokens.text_muted),
                Span::styled(text.to_string(), tokens.text_primary),
            ])])
            .wrap(Wrap { trim: false });
            f.render_widget(p, area);

            let (v_row, v_col) = calculate_visual_cursor(
                text,
                self.input.cursor_char_index(),
                prompt_prefix.chars().count() as u16,
                area.width,
            );
            let cursor_x = area.x.saturating_add(v_col);
            let cursor_y = area.y.saturating_add(v_row);
            if cursor_x < area.right() && cursor_y < area.bottom() {
                f.set_cursor_position((cursor_x, cursor_y));
            }
        }

        self.render_autocomplete(f, f.area(), area, tokens);
    }

    /// Compact suggestion popup: viewport-bounded, cursor-anchored, scrollable.
    fn render_autocomplete(
        &self,
        f: &mut Frame,
        terminal: Rect,
        composer_area: Rect,
        tokens: &ThemeTokens,
    ) {
        if !self.is_autocomplete_open || self.autocomplete_items.is_empty() {
            return;
        }
        let Some(popup_area) = self.popup_rect(terminal, composer_area) else {
            return;
        };
        if popup_area.width < 20 || popup_area.height < 3 {
            return;
        }
        f.render_widget(Clear, popup_area);

        let visible_rows = (popup_area.height.saturating_sub(2) as usize)
            .min(Self::POPUP_MAX_ROWS)
            .min(self.autocomplete_items.len())
            .max(1);
        // Keep selection visible inside this frame's window.
        let mut start = self
            .autocomplete_scroll
            .min(self.autocomplete_items.len().saturating_sub(visible_rows));
        if self.autocomplete_selected < start {
            start = self.autocomplete_selected;
        } else if self.autocomplete_selected >= start + visible_rows {
            start = self.autocomplete_selected + 1 - visible_rows;
        }
        let max_label = self
            .autocomplete_items
            .iter()
            .skip(start)
            .take(visible_rows)
            .map(|s| s.label.chars().count())
            .max()
            .unwrap_or(COMPOSER_EMPTY_LABEL_WIDTH);
        let (label_w, desc_w) = Self::suggestion_columns(popup_area.width, max_label);

        let mut lines = Vec::new();
        for (offset, item) in self
            .autocomplete_items
            .iter()
            .skip(start)
            .take(visible_rows)
            .enumerate()
        {
            let idx = start + offset;
            let is_sel = idx == self.autocomplete_selected;
            let marker = if is_sel { "› " } else { "  " };
            let label = truncate_chars(&item.label, label_w);
            let desc = truncate_chars(&item.description, desc_w);
            let label_padded = pad_to_width(&label, label_w);
            let row = format!("{marker}{label_padded} {desc}");
            let row = truncate_chars(&row, popup_area.width.saturating_sub(2) as usize);
            lines.push(ratatui::text::Line::styled(
                row,
                if is_sel {
                    tokens.selection.patch(tokens.text_primary)
                } else {
                    tokens.text_secondary
                },
            ));
        }
        // Scroll indicator when the list overflows the window. It never
        // evicts the last visible suggestion: with a single visible row the
        // selected suggestion itself is the content (an indicator-only
        // popup would be another empty shell).
        if self.autocomplete_items.len() > visible_rows && visible_rows >= 2 {
            let info = format!(
                "  … {}/{}",
                self.autocomplete_selected + 1,
                self.autocomplete_items.len()
            );
            let info = truncate_chars(&info, popup_area.width.saturating_sub(2) as usize);
            if lines.len() == visible_rows {
                lines.pop();
            }
            lines.push(ratatui::text::Line::styled(info, tokens.text_muted));
        }

        let title = match self.autocomplete_kind {
            Some(AutocompleteKind::SlashCommand) => " Commands ",
            Some(AutocompleteKind::Mention) => " Files ",
            None => " Suggestions ",
        };
        let popup_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(tokens.border_focused);
        let popup_paragraph = Paragraph::new(lines).block(popup_block);
        f.render_widget(popup_paragraph, popup_area);
    }
}

/// Unicode-safe truncation to `max_chars` display cells.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    let total_width = s.width();
    if total_width <= max_chars {
        return s.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let target_width = max_chars.saturating_sub(1);
    let mut current_width = 0;
    let mut kept = String::new();
    for g in s.graphemes(true) {
        let w = g.width();
        if current_width + w > target_width {
            break;
        }
        kept.push_str(g);
        current_width += w;
    }
    format!("{kept}…")
}

/// Pad `s` with spaces to exactly `width` display cells (no-op when already wide).
fn pad_to_width(s: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let total_width = s.width();
    if total_width >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + (width - total_width));
    out.push_str(s);
    for _ in total_width..width {
        out.push(' ');
    }
    out
}

/// Compute the visual (row, col) cursor position for text inside an area of `max_width`,
/// Compute the visual (row, col) cursor position for text inside an area of `max_width`,
/// prepended by `prefix_width` columns on the first line.
///
/// Accurately handles unicode grapheme cluster widths (`unicode-width` / `unicode-segmentation`),
/// emojis, CJK glyphs, tabs, visual wrapping (matching Ratatui's `Wrap { trim: false }`), and newline characters.
pub fn calculate_visual_cursor(
    text: &str,
    cursor_char_idx: usize,
    prefix_width: u16,
    max_width: u16,
) -> (u16, u16) {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    let max_w = if max_width == 0 {
        usize::MAX
    } else {
        max_width as usize
    };
    let mut visual_row: usize = 0;
    let mut visual_col: usize = prefix_width as usize;

    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let target_idx = cursor_char_idx.min(graphemes.len());

    for (i, g) in graphemes.iter().enumerate() {
        if i == target_idx {
            // Found cursor insertion point
            if visual_col >= max_w && max_w > 0 {
                visual_row += 1;
                visual_col = 0;
            }
            return (visual_row as u16, visual_col as u16);
        }

        if *g == "\n" {
            visual_row += 1;
            visual_col = 0;
        } else {
            let cw = if *g == "\t" {
                4 - (visual_col % 4)
            } else {
                UnicodeWidthStr::width(*g)
            };

            if visual_col + cw > max_w && max_w > 0 {
                visual_row += 1;
                visual_col = 0;
            }
            visual_col += cw;
        }
    }

    if visual_col >= max_w && max_w > 0 {
        visual_row += 1;
        visual_col = 0;
    }

    (visual_row as u16, visual_col as u16)
}

/// Strict ceiling on directory entries visited during file collection to bound synchronous filesystem work (M31A-UI-12).
pub const MAX_FS_VISITED_ENTRIES: usize = 1000;

/// Recursively collect relative file paths from the workspace up to depth 4,
/// ignoring common build directories, hidden folders, and symlinks.
/// Bounds filesystem traversal using an entry budget to prevent blocking UI.
fn collect_workspace_files(
    root: &Path,
    current_dir: &Path,
    output: &mut Vec<String>,
    depth: usize,
    budget: &mut usize,
) {
    if depth > 4 || output.len() >= 300 || *budget == 0 {
        return;
    }

    let entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if output.len() >= 300 || *budget == 0 {
            break;
        }
        *budget = budget.saturating_sub(1);
        let path = entry.path();
        // Skip symlinks to prevent traversal outside root or recursive cycles
        if path.is_symlink() {
            continue;
        }
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Skip hidden files, target, git, node_modules, dist, build
        if name_str.starts_with('.')
            || name_str == "target"
            || name_str == "node_modules"
            || name_str == "dist"
            || name_str == "build"
        {
            continue;
        }

        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            if path.is_file() {
                output.push(rel_str);
            } else if path.is_dir() {
                output.push(format!("{}/", rel_str));
                collect_workspace_files(root, &path, output, depth + 1, budget);
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

    #[test]
    fn test_composer_model_autocomplete_empty_catalog_has_no_synthetic_fallback() {
        let mut composer = TuiComposer::new(PathBuf::from("."));
        composer.set_cached_models(Vec::new());
        assert!(composer.cached_models().is_empty());

        composer.set_text("/model ");
        assert!(composer.is_autocomplete_open());
        assert_eq!(
            composer.autocomplete_kind,
            Some(AutocompleteKind::SlashCommand)
        );

        // When cache is empty, suggestions must NOT include synthetic models.
        // It must show explicit "(no models available)" placeholder.
        let suggestions = composer.autocomplete_suggestions();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].label, "(no models available)");
        assert_eq!(suggestions[0].insert_text, "/model ");

        // Verify none of the previously hardcoded synthetic models appear
        for s in suggestions {
            assert_ne!(s.label, "meta/llama-3.1-70b-instruct");
            assert_ne!(s.label, "meta/llama-3.2-11b-vision-instruct");
            assert_ne!(s.label, "meta/llama-3.3-70b-instruct");
            assert_ne!(s.label, "deepseek-ai/deepseek-r1");
        }
    }

    #[test]
    fn test_composer_model_autocomplete_populates_from_catalog() {
        let mut composer = TuiComposer::new(PathBuf::from("."));
        composer.set_cached_models(vec![
            (
                "meta/llama-3.3-70b-instruct".into(),
                "Meta Llama 3.3".into(),
            ),
            (
                "nvidia/nemotron-4-340b-instruct".into(),
                "Nemotron 4".into(),
            ),
        ]);

        composer.set_text("/model ");
        assert!(composer.is_autocomplete_open());
        let suggestions = composer.autocomplete_suggestions();
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].label, "meta/llama-3.3-70b-instruct");
        assert_eq!(
            suggestions[0].insert_text,
            "/model meta/llama-3.3-70b-instruct"
        );
        assert_eq!(suggestions[1].label, "nvidia/nemotron-4-340b-instruct");
        assert_eq!(
            suggestions[1].insert_text,
            "/model nvidia/nemotron-4-340b-instruct"
        );

        // Filter by typing a prefix
        composer.set_text("/model nvi");
        let suggestions = composer.autocomplete_suggestions();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].label, "nvidia/nemotron-4-340b-instruct");
    }
}
