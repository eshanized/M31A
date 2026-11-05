//! Bottom status bar and multiline chat composer layout controller.
//!
//! Manages composer height scaling, status pills (model, mode, tokens), and action key hints.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use unicode_width::UnicodeWidthStr;

use crate::model_selector::ModelListState;
use crate::theme::{ColorRole, Theme, theme};

/// Candidate entry in slash command autocomplete popup.
#[derive(Debug, Clone, PartialEq)]
pub struct SlashCommandCandidate {
    pub name: String,
    pub description: String,
    pub usage: String,
}

/// Single source of truth for canonical slash commands, re-exported from m31a-composer.
pub use m31a_composer::slash::{BUILTIN_SLASH_COMMANDS, CANONICAL_SLASH_COMMANDS};

/// Model choice entry for model quick-switcher popup.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelChoice {
    pub slug: String,
    pub display_name: String,
    pub context_window: usize,
    pub supports_tools: bool,
    pub supports_reasoning: bool,
}

/// Git status indicators for bottom pane badge.
#[derive(Debug, Clone, PartialEq)]
pub struct GitStatusBadge {
    pub branch: String,
    pub is_dirty: bool,
    pub modified_count: usize,
}

impl GitStatusBadge {
    pub fn new(branch: impl Into<String>, is_dirty: bool, modified_count: usize) -> Self {
        Self {
            branch: branch.into(),
            is_dirty,
            modified_count,
        }
    }

    /// Renders styled Git status badge span.
    pub fn render_span(&self) -> Span<'static> {
        let t = theme();
        if self.is_dirty {
            Span::styled(
                format!("  {}* ({} dirty) ", self.branch, self.modified_count),
                t.style_warning(true),
            )
        } else {
            Span::styled(format!("  {} ", self.branch), t.style_success(false))
        }
    }
}

/// Token context budget meter for bottom pane badge.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenContextMeter {
    pub used_tokens: usize,
    pub limit_tokens: usize,
}

impl TokenContextMeter {
    pub fn new(used_tokens: usize, limit_tokens: usize) -> Self {
        Self {
            used_tokens,
            limit_tokens: limit_tokens.max(1),
        }
    }

    pub fn percentage(&self) -> f32 {
        (self.used_tokens as f32 / self.limit_tokens as f32) * 100.0
    }

    /// Renders styled token meter span.
    pub fn render_span(&self) -> Span<'static> {
        let t = theme();
        let pct = self.percentage();
        let style = if pct >= 90.0 {
            t.style_error(false)
        } else if pct >= 75.0 {
            t.style_warning(false)
        } else {
            t.style_info(false)
        };

        let used_k = self.used_tokens as f32 / 1000.0;
        let limit_k = self.limit_tokens as f32 / 1000.0;
        Span::styled(format!(" [{used_k:.1}k/{limit_k:.0}k ({pct:.0}%)] "), style)
    }
}

/// Category of mention candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionKind {
    File,
    Directory,
    Symbol,
    Skill,
}

impl MentionKind {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Directory => "DIR",
            Self::Symbol => "SYM",
            Self::Skill => "SKILL",
        }
    }
}

/// Rich candidate for @file, @dir, @symbol, and $skill autocompletion.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionCandidate {
    pub display_name: String,
    pub insert_text: String,
    pub kind: MentionKind,
    pub detail: Option<String>,
}

impl MentionCandidate {
    pub fn new(display: impl Into<String>, insert: impl Into<String>, kind: MentionKind) -> Self {
        Self {
            display_name: display.into(),
            insert_text: insert.into(),
            kind,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl std::ops::Deref for MentionCandidate {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.insert_text
    }
}

impl std::fmt::Display for MentionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.insert_text)
    }
}

/// Truncates a long path in the middle (e.g. `crates/.../terminal.rs`) to fit compact TUI popups.
pub fn truncate_path_center(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 10 {
        return path.to_string();
    }
    let keep = (max_len - 3) / 2;
    format!("{}...{}", &path[..keep], &path[path.len() - keep..])
}

/// Execution status of a subagent for the status strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentStatus {
    Idle,
    Thinking,
    RunningTool,
    AwaitingApproval,
    Completed,
}

impl SubagentStatus {
    pub fn badge(&self) -> (&'static str, &'static str) {
        match self {
            Self::Idle => ("○", "Idle"),
            Self::Thinking => ("⠋", "Thinking"),
            Self::RunningTool => ("⚙", "Tool"),
            Self::AwaitingApproval => ("⚠", "Approval"),
            Self::Completed => ("✓", "Done"),
        }
    }
}

/// Active child subagent tab.
#[derive(Debug, Clone, PartialEq)]
pub struct SubagentTab {
    pub thread_id: String,
    pub role: String,
    pub status: SubagentStatus,
}

/// Interaction modes for the bottom command bar.
#[derive(Debug, Clone, PartialEq)]
pub enum BottomPaneMode {
    /// Normal message input.
    InputNormal,
    /// Slash command autocomplete popup is active.
    SlashCommandPopup {
        selected_index: usize,
        candidates: Vec<SlashCommandCandidate>,
    },
    /// Model quick-switcher popup is active.
    ModelSelectorPopup {
        selected_index: usize,
        models: Vec<ModelChoice>,
    },
    /// File mention autocomplete popup is active (@path/to/file or @symbol).
    FileMentionPopup {
        selected_index: usize,
        at_index: usize,
        candidates: Vec<MentionCandidate>,
    },
    /// Skill mention autocomplete popup is active ($skill-name).
    SkillMentionPopup {
        selected_index: usize,
        dollar_index: usize,
        candidates: Vec<MentionCandidate>,
    },
}

/// Output event when user submits an action in the bottom pane.
#[derive(Debug, Clone, PartialEq)]
pub enum BottomPaneSubmit {
    Message(String),
    SlashCommand(String),
    ModelSelected(String),
}

/// Helper to scan local workspace directory for file and symbol mention autocompletion (@file, @dir, @symbol).
fn find_file_candidates(query: &str, limit: usize) -> Vec<MentionCandidate> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // 1. Collect workspace files and directories with fuzzy search
    let ws_candidates = m31a_fuzzy::WorkspaceScanner::collect_workspace_files_cached(&cwd, 400);
    let matched = m31a_fuzzy::WorkspaceScanner::search(query, &ws_candidates);

    for cand in matched.into_iter().take(limit) {
        let kind = match cand.candidate_type {
            m31a_fuzzy::CandidateType::Directory => MentionKind::Directory,
            m31a_fuzzy::CandidateType::Symbol => MentionKind::Symbol,
            _ => MentionKind::File,
        };
        let disp = truncate_path_center(&cand.display_name, 38);
        results.push(MentionCandidate::new(disp, cand.insert_text, kind));
    }

    // 2. If query >= 2 and results have room, scan symbols
    if query.len() >= 2 && results.len() < limit {
        for cand in &ws_candidates {
            if results.len() >= limit {
                break;
            }
            if cand.insert_text.ends_with(".rs")
                || cand.insert_text.ends_with(".ts")
                || cand.insert_text.ends_with(".py")
            {
                let file_path = cwd.join(&cand.insert_text);
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let syms = m31a_ast::AstParser::parse_file(&file_path, &content);
                    for sym in syms {
                        if sym.name.to_lowercase().contains(&query_lower) {
                            let disp = format!("{} ({})", sym.name, sym.kind.as_str());
                            let ins = sym.name.clone();
                            let detail = truncate_path_center(&cand.insert_text, 24);
                            results.push(
                                MentionCandidate::new(disp, ins, MentionKind::Symbol)
                                    .with_detail(detail),
                            );
                            if results.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

/// Helper to scan discovered skills for skill mention autocompletion ($skill).
fn find_skill_candidates(query: &str, limit: usize) -> Vec<MentionCandidate> {
    let query_lower = query.to_lowercase();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let skills = m31a_core::skills::SkillLoader::discover_skills(&cwd);
    skills
        .into_iter()
        .filter(|s| s.name.to_lowercase().contains(&query_lower))
        .take(limit)
        .map(|s| {
            MentionCandidate::new(s.name.clone(), s.name.clone(), MentionKind::Skill)
                .with_detail(s.description)
        })
        .collect()
}

/// Unified bottom pane controller.
#[derive(Debug, Clone)]
pub struct BottomPaneState {
    pub mode: BottomPaneMode,
    pub git_status: Option<GitStatusBadge>,
    pub token_meter: Option<TokenContextMeter>,
    pub active_model: String,
    pub reasoning_effort: Option<String>,
    pub sandbox_policy: String,
    pub collaboration_mode: m31a_core::prompts::CollaborationMode,
    pub todo_summary: Option<String>,
    pub goal_summary: Option<String>,
    pub subagent_tabs: Vec<SubagentTab>,
    pub active_subagent_tab: usize,
    pub stream_metrics: Option<crate::chatwidget::StreamMetricsSnapshot>,
    /// Shared model list state for proper scrolling in model selector popup.
    pub model_selector: Option<ModelListState>,
    /// Number of queued prompts awaiting execution
    pub queued_prompts_count: usize,
}

impl Default for BottomPaneState {
    fn default() -> Self {
        Self::new("nvidia/nemotron-3.5-lightning-30b-a3b", "WorkspaceWrite")
    }
}

impl BottomPaneState {
    pub fn new(active_model: impl Into<String>, sandbox_policy: impl Into<String>) -> Self {
        Self {
            mode: BottomPaneMode::InputNormal,
            git_status: None,
            token_meter: None,
            active_model: active_model.into(),
            reasoning_effort: None,
            sandbox_policy: sandbox_policy.into(),
            collaboration_mode: m31a_core::prompts::CollaborationMode::Default,
            todo_summary: None,
            goal_summary: None,
            subagent_tabs: Vec::new(),
            active_subagent_tab: 0,
            stream_metrics: None,
            model_selector: None,
            queued_prompts_count: 0,
        }
    }

    /// Toggles between Build and Plan mode for the current session.
    pub fn toggle_mode(&mut self) -> m31a_core::prompts::CollaborationMode {
        self.collaboration_mode = match self.collaboration_mode {
            m31a_core::prompts::CollaborationMode::Default => {
                m31a_core::prompts::CollaborationMode::Plan
            }
            m31a_core::prompts::CollaborationMode::Plan => {
                m31a_core::prompts::CollaborationMode::Default
            }
        };
        self.collaboration_mode
    }

    /// Returns the dynamically requested height for the bottom pane.
    /// Scales dynamically with multiline buffer content in ChatComposer to maximize composer readability.
    pub fn desired_height(&self, composer: &crate::composer::ChatComposer) -> u16 {
        let line_count = if composer.buffer.is_empty() {
            1
        } else {
            composer.buffer.split('\n').count().max(1)
        };
        let subagent_extra = if self.subagent_tabs.is_empty() { 0 } else { 1 };
        let max_h = 14 + subagent_extra;
        // 1 line status bar + 2 lines border + line_count text lines + optional subagent strip
        (line_count as u16 + 3 + subagent_extra).clamp(4, max_h)
    }

    /// Handles character input, delegating authoritative buffer mutation to ChatComposer.
    pub fn on_char(&mut self, c: char, composer: &mut crate::composer::ChatComposer) {
        let ch = if c == '\r' { '\n' } else { c };
        composer.insert_char(ch);
        self.update_autocomplete(composer);
    }

    /// Handles pasting text into the composer buffer.
    pub fn on_paste(&mut self, text: &str, composer: &mut crate::composer::ChatComposer) {
        let clean = text.replace("\r\n", "\n").replace('\r', "\n");
        composer.insert_str(&clean);
        self.update_autocomplete(composer);
    }

    /// Handles backspace with grapheme cluster awareness.
    pub fn on_backspace(&mut self, composer: &mut crate::composer::ChatComposer) {
        composer.delete_backspace();
        self.update_autocomplete(composer);
    }

    /// Handles forward delete with grapheme cluster awareness.
    pub fn on_delete(&mut self, composer: &mut crate::composer::ChatComposer) {
        composer.delete_forward();
        self.update_autocomplete(composer);
    }

    /// Moves cursor left by one grapheme cluster.
    pub fn move_cursor_left(&mut self, composer: &mut crate::composer::ChatComposer) {
        composer.move_cursor_left();
    }

    /// Moves cursor right by one grapheme cluster.
    pub fn move_cursor_right(&mut self, composer: &mut crate::composer::ChatComposer) {
        composer.move_cursor_right();
    }

    /// Re-evaluates autocomplete candidate list based on authoritative ChatComposer buffer.
    pub fn update_autocomplete(&mut self, composer: &crate::composer::ChatComposer) {
        if composer.buffer.starts_with('/') {
            let completions = m31a_composer::slash::get_slash_completions(&composer.buffer);
            let candidates = completions
                .into_iter()
                .map(|item| SlashCommandCandidate {
                    name: item.name,
                    description: item.description,
                    usage: item.display,
                })
                .collect::<Vec<_>>();

            if !candidates.is_empty() {
                self.mode = BottomPaneMode::SlashCommandPopup {
                    selected_index: 0,
                    candidates,
                };
            } else {
                self.mode = BottomPaneMode::InputNormal;
            }
        } else {
            let text_before_cursor = &composer.buffer[..composer.cursor.min(composer.buffer.len())];
            if let Some(at_idx) = text_before_cursor.rfind('@') {
                let query = &text_before_cursor[at_idx + 1..];
                if !query.contains(' ') {
                    let candidates = find_file_candidates(query, 8);
                    if !candidates.is_empty() {
                        self.mode = BottomPaneMode::FileMentionPopup {
                            selected_index: 0,
                            at_index: at_idx,
                            candidates,
                        };
                        return;
                    }
                }
            } else if let Some(dollar_idx) = text_before_cursor.rfind('$') {
                let query = &text_before_cursor[dollar_idx + 1..];
                if !query.contains(' ') {
                    let candidates = find_skill_candidates(query, 8);
                    if !candidates.is_empty() {
                        self.mode = BottomPaneMode::SkillMentionPopup {
                            selected_index: 0,
                            dollar_index: dollar_idx,
                            candidates,
                        };
                        return;
                    }
                }
            }

            if matches!(
                self.mode,
                BottomPaneMode::SlashCommandPopup { .. }
                    | BottomPaneMode::FileMentionPopup { .. }
                    | BottomPaneMode::SkillMentionPopup { .. }
            ) {
                self.mode = BottomPaneMode::InputNormal;
            }
        }
    }

    /// Checks if any interactive popup or approval prompt is active.
    pub fn is_popup_active(&self) -> bool {
        matches!(
            self.mode,
            BottomPaneMode::SlashCommandPopup { .. }
                | BottomPaneMode::ModelSelectorPopup { .. }
                | BottomPaneMode::FileMentionPopup { .. }
                | BottomPaneMode::SkillMentionPopup { .. }
        )
    }

    /// Handles tab key for autocompletion.
    pub fn on_tab(&mut self, composer: &mut crate::composer::ChatComposer) -> bool {
        match &self.mode {
            BottomPaneMode::SlashCommandPopup {
                selected_index,
                candidates,
            } => {
                if let Some(cand) = candidates.get(*selected_index) {
                    composer.set_text(&format!("{} ", cand.name));
                    self.mode = BottomPaneMode::InputNormal;
                    return true;
                }
            }
            BottomPaneMode::FileMentionPopup {
                selected_index,
                at_index,
                candidates,
            } => {
                if let Some(cand) = candidates.get(*selected_index) {
                    let before = &composer.buffer[..*at_index];
                    let after = &composer.buffer[composer.cursor.min(composer.buffer.len())..];
                    let replacement = format!("@{cand} ");
                    composer.buffer = format!("{}{}{}", before, replacement, after);
                    composer.cursor = *at_index + replacement.len();
                    self.mode = BottomPaneMode::InputNormal;
                    return true;
                }
            }
            BottomPaneMode::SkillMentionPopup {
                selected_index,
                dollar_index,
                candidates,
            } => {
                if let Some(cand) = candidates.get(*selected_index) {
                    let before = &composer.buffer[..*dollar_index];
                    let after = &composer.buffer[composer.cursor.min(composer.buffer.len())..];
                    let replacement = format!("${cand} ");
                    composer.buffer = format!("{}{}{}", before, replacement, after);
                    composer.cursor = *dollar_index + replacement.len();
                    self.mode = BottomPaneMode::InputNormal;
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// Handles arrow up key for popup candidate navigation. Returns true if consumed by popup.
    pub fn on_up(&mut self) -> bool {
        match &mut self.mode {
            BottomPaneMode::SlashCommandPopup {
                selected_index,
                candidates,
            } => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                } else if !candidates.is_empty() {
                    *selected_index = candidates.len() - 1;
                }
                true
            }
            BottomPaneMode::ModelSelectorPopup { selected_index, .. } => {
                if let Some(ref mut ms) = self.model_selector {
                    ms.select_prev();
                    *selected_index = ms.selected_index;
                }
                true
            }
            BottomPaneMode::FileMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                } else if !candidates.is_empty() {
                    *selected_index = candidates.len() - 1;
                }
                true
            }
            BottomPaneMode::SkillMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                if *selected_index > 0 {
                    *selected_index -= 1;
                } else if !candidates.is_empty() {
                    *selected_index = candidates.len() - 1;
                }
                true
            }
            BottomPaneMode::InputNormal => false,
        }
    }

    /// Handles arrow down key for popup candidate navigation. Returns true if consumed by popup.
    pub fn on_down(&mut self) -> bool {
        match &mut self.mode {
            BottomPaneMode::SlashCommandPopup {
                selected_index,
                candidates,
            } => {
                if !candidates.is_empty() {
                    *selected_index = (*selected_index + 1) % candidates.len();
                }
                true
            }
            BottomPaneMode::ModelSelectorPopup { selected_index, .. } => {
                if let Some(ref mut ms) = self.model_selector {
                    ms.select_next();
                    *selected_index = ms.selected_index;
                }
                true
            }
            BottomPaneMode::FileMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                if !candidates.is_empty() {
                    *selected_index = (*selected_index + 1) % candidates.len();
                }
                true
            }
            BottomPaneMode::SkillMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                if !candidates.is_empty() {
                    *selected_index = (*selected_index + 1) % candidates.len();
                }
                true
            }
            BottomPaneMode::InputNormal => false,
        }
    }

    /// Handles Enter key submission backed by canonical ChatComposer.
    pub fn on_enter(
        &mut self,
        composer: &mut crate::composer::ChatComposer,
    ) -> Option<BottomPaneSubmit> {
        match &self.mode {
            BottomPaneMode::SlashCommandPopup {
                selected_index,
                candidates,
            } => {
                if let Some(cand) = candidates.get(*selected_index) {
                    let cmd = cand.name.clone();
                    composer.clear();
                    self.mode = BottomPaneMode::InputNormal;
                    Some(BottomPaneSubmit::SlashCommand(cmd))
                } else {
                    None
                }
            }
            BottomPaneMode::ModelSelectorPopup { .. } => {
                if let Some(ref ms) = self.model_selector {
                    if let Some(model) = ms.get_selected_model() {
                        let slug = model.slug.clone();
                        // Authoritative activation happens in App::switch_model after validation
                        self.model_selector = None;
                        self.mode = BottomPaneMode::InputNormal;
                        Some(BottomPaneSubmit::ModelSelected(slug))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            BottomPaneMode::FileMentionPopup { .. } | BottomPaneMode::SkillMentionPopup { .. } => {
                self.on_tab(composer);
                None
            }
            BottomPaneMode::InputNormal => {
                let text = composer.submit();
                if text.is_empty() {
                    return None;
                }
                if text.starts_with('/') {
                    Some(BottomPaneSubmit::SlashCommand(text))
                } else {
                    Some(BottomPaneSubmit::Message(text))
                }
            }
        }
    }

    /// Closes any open popup and returns to InputNormal mode.
    pub fn on_escape(&mut self) {
        self.mode = BottomPaneMode::InputNormal;
    }

    /// Opens the model selector palette.
    pub fn open_model_selector(&mut self, models: Vec<ModelChoice>) {
        let provider_models: Vec<m31a_provider::models::ModelInfo> = models
            .iter()
            .map(|m| m31a_provider::models::ModelInfo {
                slug: m.slug.clone(),
                display_name: m.display_name.clone(),
                description: String::new(),
                context_window: m.context_window as u32,
                max_output_tokens: 4096,
                supports_tools: m.supports_tools,
                supports_thinking: m.supports_reasoning,
                supports_vision: false,
                ..Default::default()
            })
            .collect();

        let mut state = ModelListState::new(provider_models, self.active_model.clone());
        state.set_viewport_height(12); // Default popup height
        let initial_idx = if let Some(idx) = models.iter().position(|m| m.slug == self.active_model)
        {
            state.selected_index = idx;
            state.ensure_selected_visible();
            idx
        } else {
            0
        };

        self.model_selector = Some(state);
        self.mode = BottomPaneMode::ModelSelectorPopup {
            selected_index: initial_idx,
            models: vec![], // Models now in model_selector
        };
    }

    /// Authoritative popup rectangle for BottomPane model selector
    pub fn model_selector_popup_area(bottom_area: Rect) -> Rect {
        let popup_height = 12.min(bottom_area.y.saturating_sub(1)).max(3);
        let popup_width = 72.min(bottom_area.width.saturating_sub(4)).max(20);
        let popup_y = bottom_area.y.saturating_sub(popup_height);
        Rect::new(bottom_area.x + 2, popup_y, popup_width, popup_height)
    }

    /// Renders the bottom command pane with badges and popups backed by canonical ChatComposer.
    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        t: &Theme,
        composer: &crate::composer::ChatComposer,
    ) {
        let (subagent_area, base_area) = if !self.subagent_tabs.is_empty() {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(3)])
                .split(area);
            (Some(split[0]), split[1])
        } else {
            (None, area)
        };

        if let Some(strip) = subagent_area {
            self.render_subagent_strip(f, strip, t);
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Badges line
                Constraint::Min(2),    // Input area
            ])
            .split(base_area);

        // 1. Badges & status bar
        self.render_status_bar(f, chunks[0], t);

        // 2. Input box (approvals are owned exclusively by the modal surface)
        self.render_input_box(f, chunks[1], t, composer);

        // 3. Floating Popups if active
        match &self.mode {
            BottomPaneMode::SlashCommandPopup {
                selected_index,
                candidates,
            } => {
                self.render_slash_popup(f, area, candidates, *selected_index, t);
            }
            BottomPaneMode::ModelSelectorPopup { .. } => {
                if let Some(ref model_selector) = self.model_selector {
                    let popup_area = Self::model_selector_popup_area(area);
                    crate::model_selector::render_model_list(
                        f,
                        model_selector,
                        popup_area,
                        t,
                        "Switch Model",
                    );
                }
            }
            BottomPaneMode::FileMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                self.render_mention_popup(
                    f,
                    area,
                    candidates,
                    *selected_index,
                    "@ Mentions (Files & Symbols)",
                    t,
                );
            }
            BottomPaneMode::SkillMentionPopup {
                selected_index,
                candidates,
                ..
            } => {
                self.render_mention_popup(
                    f,
                    area,
                    candidates,
                    *selected_index,
                    "$ Skill Mentions",
                    t,
                );
            }
            _ => {}
        }
    }

    fn render_subagent_strip(&self, f: &mut Frame, area: Rect, t: &Theme) {
        let mut spans = vec![Span::styled(
            " Swarm: ",
            t.style_secondary().add_modifier(Modifier::BOLD),
        )];

        let is_main_sel = self.active_subagent_tab == 0;
        let main_style = if is_main_sel {
            t.style_selected()
        } else {
            t.style_muted()
        };
        spans.push(Span::styled(
            if is_main_sel {
                " [● Main Agent] "
            } else {
                " [○ Main Agent] "
            },
            main_style,
        ));

        for (idx, tab) in self.subagent_tabs.iter().enumerate() {
            let tab_idx = idx + 1;
            let is_sel = self.active_subagent_tab == tab_idx;
            let (icon, label) = tab.status.badge();
            let status_color = match tab.status {
                SubagentStatus::Thinking => t.color(ColorRole::Accent),
                SubagentStatus::RunningTool => t.color(ColorRole::Warning),
                SubagentStatus::AwaitingApproval => t.color(ColorRole::Error),
                SubagentStatus::Completed => t.color(ColorRole::Success),
                SubagentStatus::Idle => t.color(ColorRole::TextMuted),
            };

            let tab_style = if is_sel {
                t.style_selected()
            } else {
                Style::default().fg(status_color)
            };

            spans.push(Span::styled(
                format!(" [{} {} ({})] ", icon, tab.role, label),
                tab_style,
            ));
        }

        spans.push(Span::styled(
            " (Alt+←/→ switch) ",
            t.style_muted().add_modifier(Modifier::ITALIC),
        ));

        let p = Paragraph::new(Line::from(spans)).style(t.style_surface0());
        f.render_widget(p, area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect, t: &Theme) {
        let mut spans = Vec::new();

        // Agent Mode badge
        match self.collaboration_mode {
            m31a_core::prompts::CollaborationMode::Default => {
                spans.push(Span::styled(
                    " BUILD ",
                    Style::default()
                        .fg(t.color(ColorRole::Surface0))
                        .bg(t.color(ColorRole::Success))
                        .add_modifier(Modifier::BOLD),
                ));
            }
            m31a_core::prompts::CollaborationMode::Plan => {
                spans.push(Span::styled(
                    " PLAN ",
                    Style::default()
                        .fg(t.color(ColorRole::Surface0))
                        .bg(t.color(ColorRole::Warning))
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }
        spans.push(Span::raw(" "));

        // Model badge with reasoning effort
        let model_label = if let Some(ref effort) = self.reasoning_effort {
            format!(" [{} | {}] ", self.active_model, effort)
        } else {
            format!(" [{}] ", self.active_model)
        };
        spans.push(Span::styled(
            model_label,
            t.style_secondary().add_modifier(Modifier::BOLD),
        ));

        // Sandbox badge
        spans.push(Span::styled(
            format!(" [{}] ", self.sandbox_policy),
            t.style_info(false),
        ));

        // Queue badge
        if self.queued_prompts_count > 0 {
            spans.push(Span::styled(
                format!(" [Queue: {}] ", self.queued_prompts_count),
                Style::default()
                    .fg(t.color(ColorRole::Surface0))
                    .bg(t.color(ColorRole::Warning))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Git status
        if let Some(git) = &self.git_status {
            spans.push(git.render_span());
        }

        // Token budget
        if let Some(meter) = &self.token_meter {
            spans.push(meter.render_span());
        }

        // Performance metrics: TTFT and tok/sec badge
        if let Some(metrics) = &self.stream_metrics
            && (metrics.tokens_per_second > 0.05 || metrics.ttft_ms.is_some())
        {
            let mut metric_str = format!(" ⚡ {:.1} tok/s", metrics.tokens_per_second);
            if let Some(ttft) = metrics.ttft_ms {
                metric_str.push_str(&format!(" (TTFT {ttft}ms)"));
            }
            metric_str.push(' ');
            spans.push(Span::styled(
                metric_str,
                Style::default()
                    .fg(t.color(ColorRole::Success))
                    .bg(t.color(ColorRole::Surface1))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Todo progress badge
        if let Some(todo) = &self.todo_summary {
            spans.push(Span::styled(
                format!(" [Todo: {}] ", todo),
                Style::default()
                    .fg(t.color(ColorRole::TextPrimary))
                    .bg(t.color(ColorRole::Surface1))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Active goal badge
        if let Some(goal) = &self.goal_summary {
            spans.push(Span::styled(
                format!(" [Goal: {}] ", goal),
                Style::default()
                    .fg(t.color(ColorRole::Surface0))
                    .bg(t.color(ColorRole::Accent))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(t.style_surface0());
        f.render_widget(paragraph, area);
    }

    fn render_input_box(
        &self,
        f: &mut Frame,
        area: Rect,
        t: &Theme,
        composer: &crate::composer::ChatComposer,
    ) {
        let mode_tag = match self.collaboration_mode {
            m31a_core::prompts::CollaborationMode::Default => "[BUILD] ",
            m31a_core::prompts::CollaborationMode::Plan => "[PLAN] ",
        };
        let mode_style = t.style_secondary().add_modifier(Modifier::BOLD);
        let prompt_span = Span::styled("❯ ", t.style_accent(true));
        let prefix_len = mode_tag.chars().count() + 2; // mode_tag + "❯ "

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(t.border.default)
            .border_style(t.border_default())
            .title(Line::from(vec![
                Span::styled(" Input ", t.style_secondary().add_modifier(Modifier::BOLD)),
                Span::styled(
                    " / commands  @ files  $ skills  Shift+Enter / Alt+Enter newline  Ctrl+V paste ",
                    t.style_muted(),
                ),
            ]))
            .style(t.style_surface1());

        if composer.buffer.is_empty() {
            let ghost_span = Span::styled(
                "Type a message, '/' for commands, '@' for files, '$' for skills, '!' for shell...",
                t.style_muted().add_modifier(Modifier::ITALIC),
            );
            let input_line = Line::from(vec![
                Span::styled(mode_tag, mode_style),
                prompt_span,
                ghost_span,
            ]);
            let paragraph = Paragraph::new(vec![input_line]).block(block);
            f.render_widget(paragraph, area);

            let cursor_x = area.x + 1 + prefix_len as u16;
            let cursor_y = area.y + 1;
            if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height - 1 {
                f.set_cursor_position((cursor_x, cursor_y));
            }
        } else {
            let buffer_lines: Vec<&str> = composer.buffer.split('\n').collect();
            let before_cursor = &composer.buffer[..composer.cursor.min(composer.buffer.len())];
            let cursor_line_idx = before_cursor.matches('\n').count();
            let last_line_segment = before_cursor.split('\n').next_back().unwrap_or("");
            let cursor_col_width = UnicodeWidthStr::width(last_line_segment);

            let inner_height = (area.height.saturating_sub(2) as usize).max(1);
            let viewport_offset = if cursor_line_idx >= inner_height {
                cursor_line_idx + 1 - inner_height
            } else {
                0
            };

            let visible_slice = if viewport_offset < buffer_lines.len() {
                &buffer_lines
                    [viewport_offset..(viewport_offset + inner_height).min(buffer_lines.len())]
            } else {
                &buffer_lines[..]
            };

            let mut rendered_lines = Vec::new();
            let continuation_indent = prefix_len.saturating_sub(2);
            let continuation_prefix = format!("{:>width$}│ ", "", width = continuation_indent);

            for (rel_idx, line_str) in visible_slice.iter().enumerate() {
                let actual_idx = viewport_offset + rel_idx;
                if actual_idx == 0 {
                    rendered_lines.push(Line::from(vec![
                        Span::styled(mode_tag, mode_style),
                        prompt_span.clone(),
                        Span::raw(*line_str),
                    ]));
                } else {
                    rendered_lines.push(Line::from(vec![
                        Span::styled(continuation_prefix.clone(), t.style_muted()),
                        Span::raw(*line_str),
                    ]));
                }
            }

            let paragraph = Paragraph::new(rendered_lines).block(block);
            f.render_widget(paragraph, area);

            // Multiline cursor positioning with accurate display cell width and viewport offset
            let cursor_x = area.x + 1 + prefix_len as u16 + cursor_col_width as u16;
            let cursor_y = area.y + 1 + (cursor_line_idx.saturating_sub(viewport_offset)) as u16;
            if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height - 1 {
                f.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }

    fn render_slash_popup(
        &self,
        f: &mut Frame,
        anchor: Rect,
        candidates: &[SlashCommandCandidate],
        selected: usize,
        t: &Theme,
    ) {
        if candidates.is_empty() {
            return;
        }
        let popup_height = (candidates.len() as u16 + 2).min(12);
        let popup_width = 72.min(anchor.width.saturating_sub(4));
        let popup_y = anchor.y.saturating_sub(popup_height);
        let popup_area = Rect::new(anchor.x + 2, popup_y, popup_width, popup_height);

        f.render_widget(Clear, popup_area);

        let max_visible = (popup_height.saturating_sub(2) as usize).max(1);
        let start_idx = if selected >= max_visible {
            selected + 1 - max_visible
        } else {
            0
        };
        let end_idx = (start_idx + max_visible).min(candidates.len());

        let items: Vec<ListItem> = candidates[start_idx..end_idx]
            .iter()
            .enumerate()
            .map(|(rel_idx, cand)| {
                let actual_idx = start_idx + rel_idx;
                let is_sel = actual_idx == selected;
                let name_style = if is_sel {
                    t.style_selected()
                } else {
                    t.style_success(true)
                };
                let desc_style = if is_sel {
                    t.style_selected()
                } else {
                    t.style_muted()
                };

                let line = Line::from(vec![
                    Span::styled(format!("{:<15} ", cand.name), name_style),
                    Span::styled(&cand.description, desc_style),
                ]);
                ListItem::new(line)
            })
            .collect();

        let counter = if candidates.len() > max_visible {
            format!(
                " [{}/{}] Tab=Complete Enter=Select",
                selected + 1,
                candidates.len()
            )
        } else {
            " Tab=Complete Enter=Select".to_string()
        };

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(t.border.default)
                .border_style(Style::default().fg(t.color(ColorRole::Accent)))
                .title(Line::from(vec![
                    Span::styled(
                        " Slash Commands ",
                        t.style_secondary().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(counter, t.style_muted()),
                ]))
                .style(t.style_surface1()),
        );

        f.render_widget(list, popup_area);
    }

    fn render_mention_popup(
        &self,
        f: &mut Frame,
        anchor: Rect,
        candidates: &[MentionCandidate],
        selected: usize,
        title: &str,
        t: &Theme,
    ) {
        if candidates.is_empty() {
            return;
        }
        let popup_height = (candidates.len() as u16 + 2).min(10);
        let popup_width = 72.min(anchor.width.saturating_sub(4));
        let popup_y = anchor.y.saturating_sub(popup_height);
        let popup_area = Rect::new(anchor.x + 2, popup_y, popup_width, popup_height);

        f.render_widget(Clear, popup_area);

        let max_visible = (popup_height.saturating_sub(2) as usize).max(1);
        let start_idx = if selected >= max_visible {
            selected + 1 - max_visible
        } else {
            0
        };
        let end_idx = (start_idx + max_visible).min(candidates.len());

        let items: Vec<ListItem> = candidates[start_idx..end_idx]
            .iter()
            .enumerate()
            .map(|(rel_idx, cand)| {
                let actual_idx = start_idx + rel_idx;
                let is_sel = actual_idx == selected;
                let row_style = if is_sel {
                    t.style_selected()
                } else {
                    t.style_primary()
                };

                let badge_style = match cand.kind {
                    MentionKind::File => Style::default().fg(t.color(ColorRole::Accent)),
                    MentionKind::Directory => Style::default().fg(t.color(ColorRole::Warning)),
                    MentionKind::Symbol => Style::default().fg(t.color(ColorRole::Info)),
                    MentionKind::Skill => Style::default().fg(t.color(ColorRole::Success)),
                };

                let mut spans = vec![
                    Span::styled(if is_sel { "▸ " } else { "  " }, row_style),
                    Span::styled(
                        format!("[{}] ", cand.kind.badge()),
                        badge_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&cand.display_name, row_style),
                ];

                if let Some(detail) = &cand.detail {
                    spans.push(Span::styled(format!(" ({})", detail), t.style_muted()));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let counter = if candidates.len() > max_visible {
            format!(" [{}/{}] Tab/Enter", selected + 1, candidates.len())
        } else {
            " Tab/Enter".to_string()
        };

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(t.border.default)
                .border_style(t.border_focus())
                .title(Line::from(vec![
                    Span::styled(
                        format!(" {title} "),
                        t.style_secondary().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(counter, t.style_muted()),
                ]))
                .style(t.style_surface1()),
        );

        f.render_widget(list, popup_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottom_pane_typing_and_cursor() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();
        assert_eq!(composer.buffer, "");
        assert_eq!(composer.cursor, 0);

        pane.on_char('H', &mut composer);
        pane.on_char('i', &mut composer);
        assert_eq!(composer.buffer, "Hi");
        assert_eq!(composer.cursor, 2);

        pane.move_cursor_left(&mut composer);
        assert_eq!(composer.cursor, 1);

        pane.on_char('e', &mut composer);
        assert_eq!(composer.buffer, "Hei");
        assert_eq!(composer.cursor, 2);

        pane.on_backspace(&mut composer);
        assert_eq!(composer.buffer, "Hi");
        assert_eq!(composer.cursor, 1);
    }

    #[test]
    fn test_slash_command_autocomplete_and_tab() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();

        pane.on_char('/', &mut composer);
        pane.on_char('m', &mut composer);
        pane.on_char('o', &mut composer);

        // Should activate popup with /model candidate
        if let BottomPaneMode::SlashCommandPopup {
            candidates,
            selected_index,
        } = &pane.mode
        {
            assert_eq!(*selected_index, 0);
            assert_eq!(candidates[0].name, "/model");
        } else {
            panic!("Expected SlashCommandPopup mode");
        }

        // Tab to complete
        assert!(pane.on_tab(&mut composer));
        assert_eq!(composer.buffer, "/model ");
        assert_eq!(pane.mode, BottomPaneMode::InputNormal);
    }

    #[test]
    fn test_slash_command_enter_submission() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();
        pane.on_char('/', &mut composer);
        pane.on_char('p', &mut composer);
        pane.on_char('l', &mut composer);

        let submit = pane.on_enter(&mut composer);
        assert_eq!(
            submit,
            Some(BottomPaneSubmit::SlashCommand("/plan".to_string()))
        );
        assert_eq!(composer.buffer, "");
        assert_eq!(pane.mode, BottomPaneMode::InputNormal);
    }

    #[test]
    fn test_model_selector_palette_navigation() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();
        let models = vec![
            ModelChoice {
                slug: "meta/llama-3.3-70b-instruct".into(),
                display_name: "Llama 3.3 70B".into(),
                context_window: 128000,
                supports_tools: true,
                supports_reasoning: false,
            },
            ModelChoice {
                slug: "deepseek-ai/deepseek-r1".into(),
                display_name: "DeepSeek R1".into(),
                context_window: 64000,
                supports_tools: true,
                supports_reasoning: true,
            },
        ];

        pane.open_model_selector(models);
        if let BottomPaneMode::ModelSelectorPopup { selected_index, .. } = pane.mode {
            assert_eq!(selected_index, 0);
        } else {
            panic!("Expected ModelSelectorPopup");
        }

        pane.on_down();
        if let BottomPaneMode::ModelSelectorPopup { selected_index, .. } = pane.mode {
            assert_eq!(selected_index, 1);
        } else {
            panic!("Expected ModelSelectorPopup index 1");
        }

        let submit = pane.on_enter(&mut composer);
        assert_eq!(
            submit,
            Some(BottomPaneSubmit::ModelSelected(
                "deepseek-ai/deepseek-r1".into()
            ))
        );
        // Under transactional activation (§4.1), pane.active_model must NOT mutate prematurely on enter.
        // It remains the previous model until App::switch_model successfully commits.
        assert_eq!(pane.active_model, "meta/llama-3.3-70b-instruct");
    }

    #[test]
    fn test_bottom_pane_paste() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();
        pane.on_char('a', &mut composer);
        pane.on_paste("bcdef", &mut composer);
        assert_eq!(composer.buffer, "abcdef");
        assert_eq!(composer.cursor, 6);
    }

    #[test]
    fn test_git_status_and_token_meter_formatting() {
        let git_clean = GitStatusBadge::new("main", false, 0);
        let git_span = git_clean.render_span();
        assert!(git_span.content.contains("main"));

        let git_dirty = GitStatusBadge::new("feat/tui", true, 3);
        let dirty_span = git_dirty.render_span();
        assert!(dirty_span.content.contains("3 dirty"));

        let meter = TokenContextMeter::new(12000, 128000);
        assert_eq!(meter.percentage(), 9.375);
        let meter_span = meter.render_span();
        assert!(meter_span.content.contains("12.0k/128k"));
    }

    #[test]
    fn test_file_and_skill_mention_autocomplete() {
        let mut pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();

        // Type '@Cargo'
        pane.on_char('@', &mut composer);
        pane.on_char('C', &mut composer);
        pane.on_char('a', &mut composer);
        pane.on_char('r', &mut composer);

        if let BottomPaneMode::FileMentionPopup {
            candidates,
            at_index,
            ..
        } = &pane.mode
        {
            assert_eq!(*at_index, 0);
            assert!(candidates.iter().any(|c| c.contains("Cargo")));
        } else {
            panic!("Expected FileMentionPopup mode");
        }

        assert!(pane.on_tab(&mut composer));
        assert!(composer.buffer.starts_with("@Cargo"));
        assert_eq!(pane.mode, BottomPaneMode::InputNormal);
    }

    #[test]
    fn test_bottom_pane_desired_height_and_multiline() {
        let pane = BottomPaneState::new("meta/llama-3.3-70b-instruct", "WorkspaceWrite");
        let mut composer = crate::composer::ChatComposer::new();
        assert_eq!(pane.desired_height(&composer), 4);

        composer.buffer = "Line 1\nLine 2".to_string();
        assert_eq!(pane.desired_height(&composer), 5);

        composer.buffer = "1\n2\n3\n4\n5".to_string();
        assert_eq!(pane.desired_height(&composer), 8);

        composer.buffer = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10".to_string();
        assert_eq!(pane.desired_height(&composer), 13);

        composer.buffer = (1..=20)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(pane.desired_height(&composer), 14); // clamped to max_h = 14
    }
}
