//! Modal dialog manager for settings, model selection, diff viewing, and command palette navigation.
//!
//! Manages modal focus, backdrop dismissal, keyboard event capturing, and overlay positioning.

use crate::resume_picker::ResumePickerState;
use m31a_core::types::ToolCall;
use m31a_diff::DiffViewerState;
use m31a_elicitation::QuestionItem;
use m31a_provider::models::ModelInfo;
use m31a_storage::db::ThreadRecord;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders};

#[derive(Debug, Clone, PartialEq)]
pub struct CommandPaletteItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub shortcut: Option<String>,
    pub icon: crate::icons::Icon,
}

/// Authoritative geometry contract for the command palette modal across renderer, mouse, and keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteGeometry {
    pub outer: Rect,
    pub search: Rect,
    pub list: Rect,
    pub details: Option<Rect>,
    pub footer: Rect,
}

impl CommandPaletteGeometry {
    pub fn compute(outer: Rect) -> Self {
        if outer.width < 10 || outer.height < 4 {
            return Self {
                outer,
                search: Rect::default(),
                list: outer,
                details: None,
                footer: Rect::default(),
            };
        }

        let inner = Block::default().borders(Borders::ALL).inner(outer);
        if inner.width < 8 || inner.height < 3 {
            return Self {
                outer,
                search: Rect::default(),
                list: inner,
                details: None,
                footer: Rect::default(),
            };
        }

        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search bar
                Constraint::Min(3),    // Body area
                Constraint::Length(1), // Footer
            ])
            .split(inner);

        let search = vert_chunks[0];
        let body = vert_chunks[1];
        let footer = vert_chunks[2];

        // Two-pane breakpoint: requires at least 70 columns of width in the body
        let is_wide = body.width >= 70;
        let (list, details) = if is_wide {
            let h_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(body);
            (h_chunks[0], Some(h_chunks[1]))
        } else {
            (body, None)
        };

        Self {
            outer,
            search,
            list,
            details,
            footer,
        }
    }

    pub fn list_inner(&self) -> Rect {
        Block::default().borders(Borders::ALL).inner(self.list)
    }

    pub fn list_inner_height(&self) -> usize {
        self.list_inner().height as usize
    }

    pub fn list_inner_width(&self) -> usize {
        self.list_inner().width as usize
    }

    /// Maps a screen click coordinate (col, row) directly to a visible row index (0..list_inner_height),
    /// strictly enforcing that clicks outside the selectable inner list rows return None.
    pub fn row_to_visible_index(&self, col: u16, row: u16) -> Option<usize> {
        let inner = self.list_inner();
        let pos = ratatui::layout::Position::new(col, row);
        if !inner.contains(pos) {
            return None;
        }
        let rel_row = (row.saturating_sub(inner.y)) as usize;
        if rel_row < self.list_inner_height() {
            Some(rel_row)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerDisplay {
    pub name: String,
    pub command: String,
    pub transport: String,
    pub tools_count: usize,
    pub tools: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectPickerItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub shortcut: Option<String>,
    pub badge: Option<String>,
    pub detail: Option<String>,
}

impl SelectPickerItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            category: category.into(),
            shortcut: None,
            badge: None,
            detail: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Generic stateful picker powering fuzzy selection with category grouping and detail previews.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectPickerState {
    pub title: String,
    pub query: String,
    pub selected_index: usize,
    pub items: Vec<SelectPickerItem>,
    pub filtered_indices: Vec<usize>,
}

impl SelectPickerState {
    pub fn new(title: impl Into<String>, items: Vec<SelectPickerItem>) -> Self {
        let indices = (0..items.len()).collect();
        Self {
            title: title.into(),
            query: String::new(),
            selected_index: 0,
            items,
            filtered_indices: indices,
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.refilter();
    }

    pub fn on_char(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn on_backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_indices.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.filtered_indices.len() - 1;
            }
        }
    }

    pub fn selected_item(&self) -> Option<&SelectPickerItem> {
        let actual_idx = *self.filtered_indices.get(self.selected_index)?;
        self.items.get(actual_idx)
    }

    pub fn refilter(&mut self) {
        if self.query.trim().is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            let q = self.query.to_lowercase();
            let mut scored: Vec<(usize, i32)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    let title = item.title.to_lowercase();
                    let desc = item.description.to_lowercase();
                    let cat = item.category.to_lowercase();
                    let shortcut = item.shortcut.as_deref().unwrap_or("").to_lowercase();

                    let mut score = 0;
                    if title == q {
                        score += 200;
                    } else if title.starts_with(&q) {
                        score += 100;
                    } else if title.contains(&q) {
                        score += 50;
                    }

                    if shortcut == q {
                        score += 150;
                    } else if shortcut.contains(&q) {
                        score += 40;
                    }

                    if cat.contains(&q) {
                        score += 20;
                    }

                    if desc.contains(&q) {
                        score += 10;
                    }

                    if score > 0 { Some((idx, score)) } else { None }
                })
                .collect();

            scored.sort_by_key(|a| std::cmp::Reverse(a.1));
            self.filtered_indices = scored.into_iter().map(|(idx, _)| idx).collect();
        }

        if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = 0;
        }
    }
}

/// A layered modal stack that manages opening, nesting, and Esc-backtracking
/// for overlay dialogs without losing parent state or context.
#[derive(Debug, Clone, Default)]
pub struct ModalStack {
    pub stack: Vec<ModalState>,
}

impl ModalStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, modal: ModalState) {
        self.stack.push(modal);
    }

    pub fn pop(&mut self) -> Option<ModalState> {
        self.stack.pop()
    }

    pub fn replace(&mut self, modal: ModalState) {
        self.stack.pop();
        self.stack.push(modal);
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn current(&self) -> Option<&ModalState> {
        self.stack.last()
    }

    pub fn current_mut(&mut self) -> Option<&mut ModalState> {
        self.stack.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: crate::icons::Icon,
    pub prompt_directive: &'static str,
}

impl CommandPaletteItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        shortcut: Option<impl Into<String>>,
    ) -> Self {
        let id_str = id.into();
        let icon = Self::default_icon_for_id(&id_str);
        Self {
            id: id_str,
            title: title.into(),
            description: description.into(),
            category: category.into(),
            shortcut: shortcut.map(|s| s.into()),
            icon,
        }
    }

    pub fn with_icon(mut self, icon: crate::icons::Icon) -> Self {
        self.icon = icon;
        self
    }

    pub fn default_icon_for_id(id: &str) -> crate::icons::Icon {
        match id {
            "nav_chat" | "chat" => crate::icons::Icon::MessageSquare,
            "nav_sessions" | "sessions" | "resume_picker" => crate::icons::Icon::History,
            "nav_models" | "models" | "model_picker" => crate::icons::Icon::Cpu,
            "nav_providers" | "providers" => crate::icons::Icon::Plug,
            "nav_agents" | "agents" => crate::icons::Icon::Bot,
            "nav_workspace" | "workspace" => crate::icons::Icon::Folder,
            "nav_source_control" | "source_control" => crate::icons::Icon::GitBranch,
            "git_diff" => crate::icons::Icon::GitCompare,
            "nav_tools" | "tools" => crate::icons::Icon::Wrench,
            "nav_settings" | "settings" => crate::icons::Icon::Settings,
            "nav_diagnostics" | "diagnostics" | "doctor" => crate::icons::Icon::Activity,
            "session_new" => crate::icons::Icon::Sparkles,
            "worktrees" => crate::icons::Icon::GitBranch,
            "cycle_reasoning" => crate::icons::Icon::Sliders,
            "test_provider" => crate::icons::Icon::Zap,
            "mode_toggle" => crate::icons::Icon::Sliders,
            "toggle_inspector" => crate::icons::Icon::Layers,
            "panel_tab_files" => crate::icons::Icon::FolderOpen,
            "panel_tab_git" => crate::icons::Icon::GitBranch,
            "panel_tab_diff" => crate::icons::Icon::GitCompare,
            "panel_tab_term" => crate::icons::Icon::Terminal,
            "panel_tab_context" => crate::icons::Icon::Context,
            "panel_tab_activity" => crate::icons::Icon::Clock,
            "panel_tab_subagents" => crate::icons::Icon::Bot,
            "panel_tab_diagnostics" => crate::icons::Icon::Chart,
            "panel_widen" | "panel_narrow" => crate::icons::Icon::ArrowUpDown,
            "help" => crate::icons::Icon::HelpCircle,
            "undo" => crate::icons::Icon::History,
            "mcp_hub" => crate::icons::Icon::Plug,
            "skills" => crate::icons::Icon::FileCode,
            "templates" => crate::icons::Icon::Layers,
            "personas" => crate::icons::Icon::User,
            "export_html" | "export_json" => crate::icons::Icon::FileText,
            "security_audit" => crate::icons::Icon::Shield,
            "active_locks" => crate::icons::Icon::Lock,
            _ => crate::icons::Icon::Command,
        }
    }
}

/// Turn entry for Esc-backtracking scrubber.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrubTurnItem {
    pub turn_index: usize,
    pub user_prompt: String,
    pub assistant_preview: Option<String>,
    pub checkpoint_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub tool_call: ToolCall,
    pub reason: String,
    /// Command that will run, when the tool is a shell execution.
    pub command: Option<String>,
    /// Rule offered by the "allow rule" action, derived once at construction.
    pub suggested_rule: Option<String>,
    /// File offered by the "allow this file for the session" action.
    pub allow_file: Option<String>,
    /// Full patch text offered by the "view diff" action.
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalActionKind {
    AllowOnce,
    AllowRule,
    AllowFile,
    ViewDiff,
    DeclineWithFeedback,
    Cancel,
}

impl PendingApproval {
    /// Builds the canonical approval payload, deriving every optional action once.
    ///
    /// Rendering, keyboard dispatch and mouse hit-testing all read these fields, so an
    /// action is offered exactly when the data required to honor it exists.
    pub fn new(tool_call: ToolCall, reason: impl Into<String>, command: Option<String>) -> Self {
        let parsed_args =
            serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments).ok();
        let command = command.or_else(|| {
            parsed_args
                .as_ref()
                .and_then(|v| v.get("command"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string())
        });
        let suggested_rule = command.as_deref().and_then(suggest_rule_from_command);
        let allow_file = parsed_args
            .as_ref()
            .and_then(|v| v.get("path").or_else(|| v.get("file_path")))
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let diff = if tool_call.function.name == "apply_patch" {
            parsed_args
                .as_ref()
                .and_then(|v| v.get("patch"))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Self {
            tool_call,
            reason: reason.into(),
            command,
            suggested_rule,
            allow_file,
            diff,
        }
    }

    /// The exact action sequence presented by the approval surface.
    ///
    /// This is the single authority for renderer rows, keyboard shortcuts, mouse
    /// hit-testing, and Enter dispatch. It contains only actions whose data exists.
    pub fn action_kinds(&self) -> Vec<ApprovalActionKind> {
        let mut list = vec![ApprovalActionKind::AllowOnce];
        if self.suggested_rule.is_some() {
            list.push(ApprovalActionKind::AllowRule);
        }
        if self.allow_file.is_some() {
            list.push(ApprovalActionKind::AllowFile);
        }
        if self.diff.is_some() {
            list.push(ApprovalActionKind::ViewDiff);
        }
        list.push(ApprovalActionKind::DeclineWithFeedback);
        list.push(ApprovalActionKind::Cancel);
        list
    }

    pub fn suggested_prefix(&self) -> Option<String> {
        self.suggested_rule.clone()
    }

    pub fn target_file(&self) -> Option<String> {
        self.allow_file.clone()
    }

    pub fn diff_text(&self) -> Option<String> {
        self.diff.clone()
    }
}

/// Shortcut shown next to an approval action. Kept next to the action enum so the
/// renderer, keyboard handler and tests cannot drift apart.
pub fn approval_action_shortcut(kind: ApprovalActionKind) -> &'static str {
    match kind {
        ApprovalActionKind::AllowOnce => "y",
        ApprovalActionKind::AllowRule => "a",
        ApprovalActionKind::AllowFile => "s",
        ApprovalActionKind::ViewDiff => "v",
        ApprovalActionKind::DeclineWithFeedback => "n",
        ApprovalActionKind::Cancel => "Esc",
    }
}

fn suggest_rule_from_command(cmd: &str) -> Option<String> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    if tokens.len() >= 2 && !tokens[1].starts_with('-') {
        if tokens.len() >= 3 && tokens[0] == "npm" && tokens[1] == "run" {
            Some(format!("{} {} {}", tokens[0], tokens[1], tokens[2]))
        } else {
            Some(format!("{} {}", tokens[0], tokens[1]))
        }
    } else {
        Some(tokens[0].to_string())
    }
}

/// The one approval popup rectangle shared by the renderer, keyboard focus and mouse.
pub fn approval_popup_rect(area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(56),
            Constraint::Percentage(22),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(14),
            Constraint::Percentage(72),
            Constraint::Percentage(14),
        ])
        .split(popup_layout[1])[1]
}

/// Area inside the approval popup border.
pub fn approval_inner_rect(popup: Rect) -> Rect {
    Rect {
        x: popup.x.saturating_add(1),
        y: popup.y.saturating_add(1),
        width: popup.width.saturating_sub(2),
        height: popup.height.saturating_sub(2),
    }
}

/// Authoritative rectangles for approval actions, bottom-aligned inside the popup.
///
/// Renderer text and mouse hit-testing both consume this list, so a click can only
/// resolve to an action that is actually displayed and available.
pub fn approval_action_rects(
    area: Rect,
    approval: &PendingApproval,
) -> Vec<(Rect, ApprovalActionKind)> {
    let inner = approval_inner_rect(approval_popup_rect(area));
    let actions = approval.action_kinds();
    if inner.height == 0 || inner.width == 0 {
        return Vec::new();
    }
    let count = actions.len().min(inner.height as usize) as u16;
    let start_y = inner.y + inner.height.saturating_sub(count);
    actions
        .into_iter()
        .take(count as usize)
        .enumerate()
        .map(|(index, kind)| {
            (
                Rect::new(inner.x, start_y + index as u16, inner.width, 1),
                kind,
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum ModalState {
    ModelPicker {
        state: crate::model_selector::ModelListState,
    },
    ApprovalDialog {
        approval: PendingApproval,
        selected_option: usize, // 0: Approve, 1: Deny, 2: Always Approve
    },
    /// Interactive decline dialog collecting feedback to feed back into agent context
    DeclineFeedbackInput {
        approval: PendingApproval,
        feedback_buffer: String,
    },
    /// Fullscreen quick diff viewer launched from approval dialog
    QuickDiffPreview {
        diff_text: Vec<String>,
        return_to_approval: PendingApproval,
        scroll: usize,
    },
    QuestionPrompt(QuestionItem),
    AnalyticsDashboard,
    InteractiveDiff(DiffViewerState),
    ResumePicker(ResumePickerState),
    DiffViewer {
        diff_text: Vec<String>,
        scroll: usize,
    },
    SymbolOutlineModal {
        file_path: String,
        symbols: Vec<m31a_core::SymbolItem>,
        selected_index: usize,
    },
    CriticalPathModal {
        durations: m31a_core::CausalPathDurations,
        completed_intervals: Vec<m31a_core::CompletedWork>,
        scroll: usize,
    },
    ActiveLocksModal {
        locks: Vec<m31a_core::LockEntry>,
        selected_index: usize,
    },
    CheckpointsModal {
        checkpoints: Vec<m31a_storage::CheckpointRecord>,
        selected_index: usize,
    },
    RevertConfirmModal {
        checkpoint: m31a_storage::CheckpointRecord,
        selected_option: usize,
    },
    ProcessesModal {
        processes: Vec<m31a_proc::ProcessInfo>,
        selected_index: usize,
    },
    ProcessViewer {
        process_id: String,
        label: String,
        lines: Vec<m31a_proc::ProcessLine>,
        scroll: usize,
    },
    HelpModal {
        scroll: usize,
    },
    /// Universal fuzzy command palette providing keyboard-driven navigation across tools, models, and actions.
    CommandPalette {
        query: String,
        selected_index: usize,
        scroll_offset: usize,
        items: Vec<CommandPaletteItem>,
        filtered_indices: Vec<usize>,
    },
    /// §7.1: experimental features toggle menu.
    ExperimentalFeatures {
        items: Vec<(
            m31a_core::features::FeatureKey,
            &'static str,
            &'static str,
            bool,
        )>,
        selected_index: usize,
    },
    /// Interactive Settings Manager for viewing and editing application configuration options.
    SettingsModal {
        active_tab: usize,
        selected_index: usize,
    },
    /// Interactive Git Worktree selector for branch-isolated agent workspaces (`Ctrl+W` / `/worktree`).
    WorktreeSelector {
        worktrees: Vec<m31a_core::worktree::ManagedWorktree>,
        selected_index: usize,
    },
    /// Model Context Protocol (MCP) server hub browsing and inspection (`/mcp`).
    McpHubModal {
        servers: Vec<McpServerDisplay>,
        selected_index: usize,
        view_tools: bool,
    },
    /// Discovered Skills library split-view modal (`/skills`).
    SkillsModal {
        skills: Vec<m31a_core::skills::SkillInfo>,
        selected_index: usize,
        search_query: String,
    },
    /// Collaboration session templates selector modal (`/template`).
    TemplatesModal {
        templates: Vec<m31a_collaboration_templates::CollaborationTemplate>,
        selected_index: usize,
    },
    /// Long-running Goal & Mission execution dashboard modal (`/goal`).
    GoalDashboardModal {
        goal: Option<m31a_storage::db::Goal>,
        scroll: usize,
    },
    /// Active child subagents and swarm hierarchy command center (`/agents`).
    SubagentsModal {
        subagents: Vec<(String, String)>,
        selected_index: usize,
        recent_posts: Vec<m31a_storage::BoardPost>,
    },
    /// Interactive Esc-backtracking turn scrubber modal.
    TurnScrubber {
        turns: Vec<ScrubTurnItem>,
        selected_index: usize,
    },
    /// Dynamic context window monitor and file pinning modal (`/context`).
    ContextManagerModal {
        pinned_items: Vec<m31a_core::DynamicContextItem>,
        world_sections: Vec<(String, String)>,
        selected_index: usize,
        active_tab: usize,
    },
    /// Selectable Engineering Persona / Workstyle modal (`/persona`).
    PersonaModal {
        personas: Vec<PersonaItem>,
        selected_index: usize,
    },
    /// Interactive V8/TypeScript Code Mode Scratchpad (`/scratchpad`).
    CodeModeScratchpad(CodeModeScratchpadState),
    /// Rollout Journal and append-only event diagnostics viewer (`/rollout`).
    RolloutJournal(RolloutJournalState),
    /// Generic unified fuzzy selector with category grouping and detail previews.
    SelectPicker(SelectPickerState),
    /// Interactive multi-step setup onboarding wizard (`/setup`).
    SetupWizard {
        state: SetupWizardState,
    },
    /// Interactive reasoning effort selection dialog (`/effort`).
    ReasoningEffort {
        state: ReasoningEffortState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchpadFocus {
    Editor,
    Viewer,
}

#[derive(Debug, Clone)]
pub struct CodeModeScratchpadState {
    pub code: String,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub focus: ScratchpadFocus,
    pub output_lines: Vec<String>,
    pub ambient_dts_lines: Vec<String>,
    pub scroll_output: usize,
    pub scroll_dts: usize,
    pub active_tab: usize, // 0: Output, 1: Ambient DTS, 2: Budgets & Engine
    pub is_evaluating: bool,
    pub max_tokens: usize,
    pub yield_ms: u64,
}

impl CodeModeScratchpadState {
    pub fn new(ambient_dts: String, initial_code: Option<String>) -> Self {
        let code = initial_code.unwrap_or_else(|| {
            "// M31A Autonomous Code Mode Scratchpad\n// Execute multi-tool scripts programmatically with typed APIs\n\nconst items = await tools.list_directory({ path: \".\" });\nprint(`Discovered ${items.length} items in workspace root:`);\nfor (const item of items.slice(0, 5)) {\n    print(` - ${item.name} (${item.is_dir ? \"dir\" : \"file\"})`);\n}\n".to_string()
        });
        let ambient_dts_lines = ambient_dts.lines().map(|s| s.to_string()).collect();
        Self {
            code,
            cursor_line: 0,
            cursor_col: 0,
            focus: ScratchpadFocus::Editor,
            output_lines: vec![
                "// Press [F5] or [Ctrl+Enter] to evaluate script in isolated sandbox..."
                    .to_string(),
            ],
            ambient_dts_lines,
            scroll_output: 0,
            scroll_dts: 0,
            active_tab: 0,
            is_evaluating: false,
            max_tokens: 4096,
            yield_ms: 10000,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let mut lines: Vec<String> = self.code.split('\n').map(|s| s.to_string()).collect();
        if self.cursor_line >= lines.len() {
            lines.resize(self.cursor_line + 1, String::new());
        }
        let line = &mut lines[self.cursor_line];
        let col = self.cursor_col.min(line.chars().count());
        let byte_idx = line
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        line.insert(byte_idx, c);
        self.cursor_col = col + 1;
        self.code = lines.join("\n");
    }

    pub fn insert_newline(&mut self) {
        let mut lines: Vec<String> = self.code.split('\n').map(|s| s.to_string()).collect();
        if self.cursor_line >= lines.len() {
            lines.resize(self.cursor_line + 1, String::new());
        }
        let line = &lines[self.cursor_line];
        let col = self.cursor_col.min(line.chars().count());
        let byte_idx = line
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        let before = line[..byte_idx].to_string();
        let after = line[byte_idx..].to_string();
        lines[self.cursor_line] = before;
        lines.insert(self.cursor_line + 1, after);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.code = lines.join("\n");
    }

    pub fn delete_backspace(&mut self) {
        let mut lines: Vec<String> = self.code.split('\n').map(|s| s.to_string()).collect();
        if self.cursor_line >= lines.len() {
            return;
        }
        if self.cursor_col > 0 {
            let line = &mut lines[self.cursor_line];
            let col = self.cursor_col.min(line.chars().count());
            let byte_idx = line
                .char_indices()
                .nth(col - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let next_byte_idx = line
                .char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            line.drain(byte_idx..next_byte_idx);
            self.cursor_col = col - 1;
            self.code = lines.join("\n");
        } else if self.cursor_line > 0 {
            let current_line = lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_line = &mut lines[self.cursor_line];
            let prev_len = prev_line.chars().count();
            prev_line.push_str(&current_line);
            self.cursor_col = prev_len;
            self.code = lines.join("\n");
        }
    }
}

#[derive(Debug, Clone)]
pub struct RolloutEventEntry {
    pub id: usize,
    pub timestamp: String,
    pub category: String, // "PROMPT", "ASSISTANT", "TOOL", "TOOL_RES", "CHECKPOINT", "METRIC", "NOTICE"
    pub title: String,
    pub detail: String,
    pub tokens: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RolloutJournalState {
    pub events: Vec<RolloutEventEntry>,
    pub selected_index: usize,
    pub filter: String, // "all", "prompts", "tools", "checkpoints"
    pub view_json: bool,
    pub scroll_detail: usize,
}

impl RolloutJournalState {
    pub fn new(events: Vec<RolloutEventEntry>) -> Self {
        Self {
            events,
            selected_index: 0,
            filter: "all".to_string(),
            view_json: false,
            scroll_detail: 0,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        self.events
            .iter()
            .enumerate()
            .filter(|(_, ev)| match self.filter.as_str() {
                "prompts" => ev.category == "PROMPT",
                "tools" => ev.category == "TOOL" || ev.category == "TOOL_RES",
                "checkpoints" => ev.category == "CHECKPOINT",
                _ => true,
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// Step progression in the interactive setup wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Welcome = 0,
    Provider = 1,
    Credentials = 2,
    Model = 3,
    Security = 4,
    Connectivity = 5,
    Review = 6,
    Complete = 7,
}

impl SetupStep {
    pub const ALL: [SetupStep; 8] = [
        SetupStep::Welcome,
        SetupStep::Provider,
        SetupStep::Credentials,
        SetupStep::Model,
        SetupStep::Security,
        SetupStep::Connectivity,
        SetupStep::Review,
        SetupStep::Complete,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome to M31A",
            Self::Provider => "Provider Configuration",
            Self::Credentials => "API Credentials",
            Self::Model => "Model Selection",
            Self::Security => "Execution Security",
            Self::Connectivity => "Connectivity Verification",
            Self::Review => "Review & Persist",
            Self::Complete => "Setup Complete",
        }
    }
}

/// Provider connectivity status in setup wizard with explicit distinguishable failure and verification states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityStatus {
    NotTested,
    Validating,
    TestingConnection,
    CheckingModel,
    Connected,
    AuthenticationFailed,
    ModelUnsupported,
    TimedOut,
    Unreachable,
    Failed,
}

impl ConnectivityStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::AuthenticationFailed
                | Self::ModelUnsupported
                | Self::TimedOut
                | Self::Unreachable
                | Self::Failed
        )
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(
            self,
            Self::Validating | Self::TestingConnection | Self::CheckingModel
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::NotTested => "Not Tested",
            Self::Validating => "Validating Configuration",
            Self::TestingConnection => "Testing Connection",
            Self::CheckingModel => "Checking Model",
            Self::Connected => "Connected",
            Self::AuthenticationFailed => "Authentication Failed",
            Self::ModelUnsupported => "Model Unsupported",
            Self::TimedOut => "Timed Out",
            Self::Unreachable => "Host Unreachable",
            Self::Failed => "Verification Failed",
        }
    }
}

/// Structured outcome of genuine provider verification probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectivityDetails {
    pub endpoint_verified: bool,
    pub model_verified: bool,
    pub model_check_unsupported: bool,
    pub latency_ms: Option<u64>,
    pub message: String,
}

/// Candidate configuration prepared by the Setup Wizard before transactional commit.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupConfiguration {
    pub provider_type: m31a_provider::ProviderType,
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub sandbox: m31a_core::SandboxPolicy,
    pub approval: m31a_core::ApprovalPolicy,
}

/// Dedicated state for interactive multi-step `/setup` onboarding wizard.
#[derive(Clone)]
pub struct SetupWizardState {
    pub step: SetupStep,
    pub existing_config_detected: bool,

    // Step 2: Provider
    pub selected_provider_idx: usize,
    pub providers: Vec<m31a_provider::ProviderType>,
    pub custom_endpoint: String,
    pub editing_endpoint: bool,

    // Step 3: Credentials
    pub api_key_input: String,
    pub has_existing_key: bool,
    pub preserve_existing_key: bool,

    // Step 4: Model
    pub models_for_provider: Vec<m31a_provider::models::ModelInfo>,
    pub selected_model_idx: usize,
    pub custom_model_name: String,
    pub editing_custom_model: bool,

    // Step 5: Security
    pub sandbox_policy: m31a_core::SandboxPolicy,
    pub approval_policy: m31a_core::ApprovalPolicy,
    pub security_field_idx: usize, // 0 = sandbox, 1 = approval

    // Step 6: Connectivity
    pub connectivity_status: ConnectivityStatus,
    pub connectivity_details: Option<ConnectivityDetails>,
    pub connectivity_error: Option<String>,
    pub connectivity_generation: u64,
    pub skipped_verification: bool,

    // Step 7 & 8: Review & Save
    pub save_error: Option<String>,
}

impl std::fmt::Debug for SetupWizardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetupWizardState")
            .field("step", &self.step)
            .field("existing_config_detected", &self.existing_config_detected)
            .field("selected_provider_idx", &self.selected_provider_idx)
            .field("providers", &self.providers)
            .field("custom_endpoint", &self.custom_endpoint)
            .field("editing_endpoint", &self.editing_endpoint)
            .field(
                "api_key_input",
                if self.api_key_input.is_empty() {
                    &"<empty>"
                } else {
                    &"[REDACTED]"
                },
            )
            .field("has_existing_key", &self.has_existing_key)
            .field("preserve_existing_key", &self.preserve_existing_key)
            .field("models_count", &self.models_for_provider.len())
            .field("selected_model_idx", &self.selected_model_idx)
            .field("custom_model_name", &self.custom_model_name)
            .field("editing_custom_model", &self.editing_custom_model)
            .field("sandbox_policy", &self.sandbox_policy)
            .field("approval_policy", &self.approval_policy)
            .field("security_field_idx", &self.security_field_idx)
            .field("connectivity_status", &self.connectivity_status)
            .field("connectivity_details", &self.connectivity_details)
            .field("connectivity_error", &self.connectivity_error)
            .field("connectivity_generation", &self.connectivity_generation)
            .field("skipped_verification", &self.skipped_verification)
            .field("save_error", &self.save_error)
            .finish()
    }
}

impl SetupWizardState {
    pub fn new(
        current_config: &m31a_core::Config,
        available_models: &[m31a_provider::models::ModelInfo],
    ) -> Self {
        let providers = vec![
            m31a_provider::ProviderType::Nim,
            m31a_provider::ProviderType::OpenAi,
            m31a_provider::ProviderType::Anthropic,
            m31a_provider::ProviderType::Gemini,
            m31a_provider::ProviderType::DeepSeek,
            m31a_provider::ProviderType::Groq,
            m31a_provider::ProviderType::Mistral,
            m31a_provider::ProviderType::Together,
            m31a_provider::ProviderType::Ollama,
            m31a_provider::ProviderType::LmStudio,
            m31a_provider::ProviderType::Custom,
        ];

        let config_file = m31a_core::Config::config_dir().join("config.toml");
        let existing_detected = config_file.exists();

        let detected_provider = m31a_provider::ProviderType::resolve(
            current_config.model_provider.as_deref(),
            &current_config.model,
            Some(&current_config.api_base),
        )
        .unwrap_or(m31a_provider::ProviderType::Nim);

        let selected_provider_idx = providers
            .iter()
            .position(|&p| p == detected_provider)
            .unwrap_or(0);

        let has_existing_key = current_config
            .api_key
            .as_ref()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);

        let mut wizard = Self {
            step: SetupStep::Welcome,
            existing_config_detected: existing_detected,
            selected_provider_idx,
            providers,
            custom_endpoint: current_config.api_base.clone(),
            editing_endpoint: false,
            api_key_input: String::new(),
            has_existing_key,
            preserve_existing_key: has_existing_key,
            models_for_provider: Vec::new(),
            selected_model_idx: 0,
            custom_model_name: current_config.model.clone(),
            editing_custom_model: false,
            sandbox_policy: current_config.sandbox,
            approval_policy: current_config.approval,
            security_field_idx: 0,
            connectivity_status: ConnectivityStatus::NotTested,
            connectivity_details: None,
            connectivity_error: None,
            connectivity_generation: 1,
            skipped_verification: false,
            save_error: None,
        };

        wizard.update_models_for_provider(available_models);
        wizard
    }

    pub fn invalidate_connectivity(&mut self) {
        self.connectivity_generation += 1;
        self.connectivity_status = ConnectivityStatus::NotTested;
        self.connectivity_details = None;
        self.connectivity_error = None;
        self.skipped_verification = false;
    }

    pub fn build_candidate_configuration(
        &self,
        current_config: &m31a_core::Config,
    ) -> SetupConfiguration {
        let api_key = if !self.api_key_input.trim().is_empty() {
            Some(self.api_key_input.trim().to_string())
        } else if self.preserve_existing_key {
            current_config.api_key.clone()
        } else {
            None
        };

        SetupConfiguration {
            provider_type: self.current_provider(),
            api_base: self.effective_endpoint(),
            api_key,
            model: self.effective_model(),
            sandbox: self.sandbox_policy,
            approval: self.approval_policy,
        }
    }

    pub fn current_provider(&self) -> m31a_provider::ProviderType {
        self.providers
            .get(self.selected_provider_idx)
            .copied()
            .unwrap_or(m31a_provider::ProviderType::Nim)
    }

    pub fn effective_endpoint(&self) -> String {
        if self.custom_endpoint.trim().is_empty() {
            self.current_provider().default_api_base().to_string()
        } else {
            self.custom_endpoint.trim().to_string()
        }
    }

    pub fn effective_model(&self) -> String {
        if self.editing_custom_model && !self.custom_model_name.trim().is_empty() {
            return self.custom_model_name.trim().to_string();
        }
        if let Some(m) = self.models_for_provider.get(self.selected_model_idx) {
            m.slug.clone()
        } else if !self.custom_model_name.trim().is_empty() {
            self.custom_model_name.trim().to_string()
        } else {
            self.current_provider().default_model().to_string()
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.step != SetupStep::Welcome && self.step != SetupStep::Complete
    }

    pub fn previous_step(&mut self) {
        match self.step {
            SetupStep::Welcome => {}
            SetupStep::Provider => self.step = SetupStep::Welcome,
            SetupStep::Credentials => self.step = SetupStep::Provider,
            SetupStep::Model => self.step = SetupStep::Credentials,
            SetupStep::Security => self.step = SetupStep::Model,
            SetupStep::Connectivity => self.step = SetupStep::Security,
            SetupStep::Review => self.step = SetupStep::Connectivity,
            SetupStep::Complete => {}
        }
    }

    pub fn next_step(&mut self) {
        match self.step {
            SetupStep::Welcome => self.step = SetupStep::Provider,
            SetupStep::Provider => self.step = SetupStep::Credentials,
            SetupStep::Credentials => self.step = SetupStep::Model,
            SetupStep::Model => self.step = SetupStep::Security,
            SetupStep::Security => self.step = SetupStep::Connectivity,
            SetupStep::Connectivity => self.step = SetupStep::Review,
            SetupStep::Review => self.step = SetupStep::Complete,
            SetupStep::Complete => {}
        }
    }

    pub fn update_models_for_provider(
        &mut self,
        catalog_models: &[m31a_provider::models::ModelInfo],
    ) {
        let provider = self.current_provider();

        // 1. Check if the catalog contains canonical models matching this provider
        let catalog = if catalog_models.is_empty() {
            m31a_provider::models::get_nvidia_model_catalog()
        } else {
            catalog_models.to_vec()
        };

        let mut matched: Vec<m31a_provider::models::ModelInfo> = match provider {
            m31a_provider::ProviderType::Nim => catalog
                .iter()
                .filter(|m| {
                    m.originator.to_lowercase().contains("nvidia")
                        || m.slug.contains("meta/")
                        || m.slug.contains("deepseek-ai/")
                        || m.slug.contains("mistralai/")
                        || m.slug.contains("qwen/")
                })
                .cloned()
                .collect(),
            _ => {
                let provider_name = provider.display_name().to_lowercase();
                catalog
                    .iter()
                    .filter(|m| {
                        m.originator.to_lowercase() == provider_name
                            || m.originator.to_lowercase().contains(&provider_name)
                            || m.slug
                                .to_lowercase()
                                .starts_with(&format!("{}/", provider_name))
                    })
                    .cloned()
                    .collect()
            }
        };

        // 2. If provider has no canonical models in the catalogue, do NOT fabricate fake
        // specifications or limits. Provide an honest fallback with unverified (0) limits
        // and allow custom model entry.
        if matched.is_empty() {
            let default_slug = provider.default_model();
            if !default_slug.is_empty() {
                matched.push(m31a_provider::models::ModelInfo {
                    slug: default_slug.to_string(),
                    display_name: format!("{} (Provider Default)", default_slug),
                    description: format!(
                        "Default model for {}. Local catalog metadata not available.",
                        provider.display_name()
                    ),
                    context_window: 0, // 0 signifies unknown / unverified
                    max_output_tokens: 0,
                    supports_tools: false,
                    supports_thinking: false,
                    supports_vision: false,
                    originator: provider.display_name().to_string(),
                    ..Default::default()
                });
            }
        }

        self.models_for_provider = matched;
        self.selected_model_idx = 0;
    }
}

/// Reasoning effort modal option description.
#[derive(Debug, Clone, PartialEq)]
pub struct ReasoningEffortOption {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub icon: crate::icons::Icon,
}

/// Dedicated state for interactive reasoning effort dialog (`/effort`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReasoningEffortState {
    pub selected_index: usize,
    pub current_setting: Option<String>,
    pub options: Vec<ReasoningEffortOption>,
}

impl ReasoningEffortState {
    pub fn new(current: Option<String>) -> Self {
        let options = vec![
            ReasoningEffortOption {
                id: "none",
                label: "None",
                description: "Use provider default thinking behavior (no reasoning override)",
                icon: crate::icons::Icon::Sliders,
            },
            ReasoningEffortOption {
                id: "low",
                label: "Low",
                description: "Minimize reasoning effort for low latency responses",
                icon: crate::icons::Icon::Zap,
            },
            ReasoningEffortOption {
                id: "medium",
                label: "Medium",
                description: "Balanced reasoning effort for general autonomous coding",
                icon: crate::icons::Icon::Sliders,
            },
            ReasoningEffortOption {
                id: "high",
                label: "High",
                description: "Maximum reasoning effort for deep multi-step architecture",
                icon: crate::icons::Icon::Sparkles,
            },
        ];

        let selected_index = match current.as_deref() {
            Some("low") => 1,
            Some("medium") | Some("med") => 2,
            Some("high") => 3,
            _ => 0,
        };

        Self {
            selected_index,
            current_setting: current,
            options,
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.options.len() {
            self.selected_index += 1;
        }
    }

    pub fn prev(&mut self) {
        self.select_prev();
    }

    pub fn next(&mut self) {
        self.select_next();
    }

    pub fn selected_value(&self) -> Option<String> {
        match self.selected_index {
            1 => Some("low".to_string()),
            2 => Some("medium".to_string()),
            3 => Some("high".to_string()),
            _ => None,
        }
    }
}

impl ModalState {
    pub fn dimensions(&self) -> (u16, u16) {
        match self {
            ModalState::ModelPicker { .. } => (72, 70),
            ModalState::ApprovalDialog { .. } => (72, 56),
            ModalState::DeclineFeedbackInput { .. } => (65, 35),
            ModalState::QuickDiffPreview { .. } => (88, 85),
            ModalState::QuestionPrompt(_) => (70, 50),
            ModalState::AnalyticsDashboard => (80, 75),
            ModalState::InteractiveDiff(_) => (90, 85),
            ModalState::ResumePicker(_) => (80, 75),
            ModalState::DiffViewer { .. } => (88, 85),
            ModalState::SymbolOutlineModal { .. } => (82, 80),
            ModalState::CriticalPathModal { .. } => (85, 80),
            ModalState::ActiveLocksModal { .. } => (75, 70),
            ModalState::CheckpointsModal { .. } => (80, 75),
            ModalState::RevertConfirmModal { .. } => (60, 30),
            ModalState::ProcessesModal { .. } => (80, 75),
            ModalState::ProcessViewer { .. } => (85, 80),
            ModalState::HelpModal { .. } => (75, 75),
            ModalState::CommandPalette { .. } => (78, 75),
            ModalState::ExperimentalFeatures { .. } => (75, 70),
            ModalState::SettingsModal { .. } => (76, 75),
            ModalState::WorktreeSelector { .. } => (84, 75),
            ModalState::McpHubModal { .. } => (82, 78),
            ModalState::SkillsModal { .. } => (84, 82),
            ModalState::TemplatesModal { .. } => (82, 78),
            ModalState::GoalDashboardModal { .. } => (76, 68),
            ModalState::SubagentsModal { .. } => (84, 76),
            ModalState::TurnScrubber { .. } => (82, 74),
            ModalState::ContextManagerModal { .. } => (82, 78),
            ModalState::PersonaModal { .. } => (76, 74),
            ModalState::CodeModeScratchpad(_) => (90, 85),
            ModalState::RolloutJournal(_) => (88, 80),
            ModalState::SelectPicker(_) => (76, 75),
            ModalState::SetupWizard { .. } => (78, 80),
            ModalState::ReasoningEffort { .. } => (60, 48),
        }
    }

    pub fn calculate_rect(&self, area: Rect) -> Rect {
        if matches!(self, ModalState::ApprovalDialog { .. }) {
            return approval_popup_rect(area);
        }
        if matches!(self, ModalState::ModelPicker { .. }) {
            return crate::renderer::model_picker_popup_rect(area);
        }
        let (percent_x, percent_y) = self.dimensions();
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    pub fn model_picker(models: Vec<ModelInfo>, current_slug: &str) -> Self {
        let mut state =
            crate::model_selector::ModelListState::new(models, current_slug.to_string());
        state.set_active_model(current_slug.to_string());
        state.set_viewport_height(15);
        Self::ModelPicker { state }
    }

    pub fn approval(tool_call: ToolCall, reason: String, command: Option<String>) -> Self {
        Self::ApprovalDialog {
            approval: PendingApproval::new(tool_call, reason, command),
            selected_option: 0,
        }
    }

    pub fn resume_picker(threads: Vec<ThreadRecord>) -> Self {
        Self::ResumePicker(ResumePickerState::new(threads))
    }

    pub fn diff_viewer(diff: String) -> Self {
        let diff_text = diff.lines().map(|s| s.to_string()).collect();
        Self::DiffViewer {
            diff_text,
            scroll: 0,
        }
    }

    pub fn symbol_outline(file_path: String, symbols: Vec<m31a_core::SymbolItem>) -> Self {
        Self::SymbolOutlineModal {
            file_path,
            symbols,
            selected_index: 0,
        }
    }

    pub fn critical_path_modal(
        durations: m31a_core::CausalPathDurations,
        completed_intervals: Vec<m31a_core::CompletedWork>,
    ) -> Self {
        Self::CriticalPathModal {
            durations,
            completed_intervals,
            scroll: 0,
        }
    }

    pub fn active_locks(locks: Vec<m31a_core::LockEntry>) -> Self {
        Self::ActiveLocksModal {
            locks,
            selected_index: 0,
        }
    }

    pub fn checkpoints_modal(checkpoints: Vec<m31a_storage::CheckpointRecord>) -> Self {
        Self::CheckpointsModal {
            checkpoints,
            selected_index: 0,
        }
    }

    pub fn revert_confirm(checkpoint: m31a_storage::CheckpointRecord) -> Self {
        Self::RevertConfirmModal {
            checkpoint,
            selected_option: 0,
        }
    }

    pub fn processes_modal(processes: Vec<m31a_proc::ProcessInfo>) -> Self {
        Self::ProcessesModal {
            processes,
            selected_index: 0,
        }
    }

    pub fn process_viewer(
        process_id: String,
        label: String,
        lines: Vec<m31a_proc::ProcessLine>,
    ) -> Self {
        Self::ProcessViewer {
            process_id,
            label,
            lines,
            scroll: 0,
        }
    }

    pub fn help() -> Self {
        Self::HelpModal { scroll: 0 }
    }

    pub fn mcp_hub(servers: Vec<McpServerDisplay>) -> Self {
        Self::McpHubModal {
            servers,
            selected_index: 0,
            view_tools: false,
        }
    }

    pub fn skills_modal(skills: Vec<m31a_core::skills::SkillInfo>) -> Self {
        Self::SkillsModal {
            skills,
            selected_index: 0,
            search_query: String::new(),
        }
    }

    pub fn templates_modal(
        templates: Vec<m31a_collaboration_templates::CollaborationTemplate>,
    ) -> Self {
        Self::TemplatesModal {
            templates,
            selected_index: 0,
        }
    }

    pub fn goal_dashboard(goal: Option<m31a_storage::db::Goal>) -> Self {
        Self::GoalDashboardModal { goal, scroll: 0 }
    }

    pub fn subagents_modal(
        subagents: Vec<(String, String)>,
        recent_posts: Vec<m31a_storage::BoardPost>,
    ) -> Self {
        Self::SubagentsModal {
            subagents,
            selected_index: 0,
            recent_posts,
        }
    }

    pub fn turn_scrubber(turns: Vec<ScrubTurnItem>) -> Self {
        Self::TurnScrubber {
            turns,
            selected_index: 0,
        }
    }

    pub fn code_mode_scratchpad(ambient_dts: String, initial_code: Option<String>) -> Self {
        Self::CodeModeScratchpad(CodeModeScratchpadState::new(ambient_dts, initial_code))
    }

    pub fn rollout_journal(events: Vec<RolloutEventEntry>) -> Self {
        Self::RolloutJournal(RolloutJournalState::new(events))
    }

    pub fn select_picker(title: impl Into<String>, items: Vec<SelectPickerItem>) -> Self {
        Self::SelectPicker(SelectPickerState::new(title, items))
    }

    pub fn context_manager(
        pinned_items: Vec<m31a_core::DynamicContextItem>,
        world_sections: Vec<(String, String)>,
    ) -> Self {
        Self::ContextManagerModal {
            pinned_items,
            world_sections,
            selected_index: 0,
            active_tab: 0,
        }
    }

    pub fn persona_modal(personas: Vec<PersonaItem>, current_id: Option<&str>) -> Self {
        let selected_index = current_id
            .and_then(|id| personas.iter().position(|p| p.id == id))
            .unwrap_or(0);
        Self::PersonaModal {
            personas,
            selected_index,
        }
    }

    pub fn default_personas() -> Vec<PersonaItem> {
        vec![
            PersonaItem {
                id: "balanced".to_string(),
                name: "Balanced Engineer (Default)".to_string(),
                description: "Pragmatic balance between speed, planning, clean implementation, and testing.".to_string(),
                icon: crate::icons::Icon::Sliders,
                prompt_directive: "Approach tasks pragmatically: balance architectural foresight, clean implementations, and focused verifications.",
            },
            PersonaItem {
                id: "architect".to_string(),
                name: "Software Architect".to_string(),
                description: "Deep modular abstractions, clean interface boundaries, and visual mermaid system diagrams.".to_string(),
                icon: crate::icons::Icon::Layers,
                prompt_directive: "Adopt an Architectural perspective: prioritize high modularity, explicit domain boundaries, clean contracts, and mermaid architecture diagrams before mutating code.",
            },
            PersonaItem {
                id: "security".to_string(),
                name: "Security Auditor".to_string(),
                description: "Paranoia-level vulnerability checks, boundary validation, least-privilege armor, and OWASP defenses.".to_string(),
                icon: crate::icons::Icon::Shield,
                prompt_directive: "Act as a Senior Security Auditor: evaluate every boundary for command injection, path traversal, auth leaks, least-privilege violations, and defensive error handling.",
            },
            PersonaItem {
                id: "perf".to_string(),
                name: "Performance Optimizer".to_string(),
                description: "Zero-cost abstractions, minimal heap allocations, fast hot paths, and algorithmic profiling.".to_string(),
                icon: crate::icons::Icon::Zap,
                prompt_directive: "Focus intensely on performance: eliminate redundant allocations, streamline hot execution loops, minimize algorithmic complexity, and profile critical latency paths.",
            },
            PersonaItem {
                id: "tdd".to_string(),
                name: "Test-Driven Engineer".to_string(),
                description: "Strict Red-Green-Refactor discipline: write failing unit/integration tests before writing production code.".to_string(),
                icon: crate::icons::Icon::Check,
                prompt_directive: "Follow rigorous Test-Driven Development (TDD): first specify requirements with thorough test assertions, run the suite to confirm failure, then write minimal clean code to make it pass.",
            },
            PersonaItem {
                id: "minimalist".to_string(),
                name: "Minimalist Coder".to_string(),
                description: "Lean, concise code avoiding extraneous abstractions, scaffolding, and bloated dependencies.".to_string(),
                icon: crate::icons::Icon::Code,
                prompt_directive: "Embody radical minimalism: write the simplest, cleanest code that satisfies the requirements without over-engineering, unnecessary abstractions, or new dependencies.",
            },
        ]
    }

    pub fn settings() -> Self {
        Self::SettingsModal {
            active_tab: 0,
            selected_index: 0,
        }
    }

    pub fn setup_wizard(state: SetupWizardState) -> Self {
        Self::SetupWizard { state }
    }

    pub fn reasoning_effort(current: Option<String>) -> Self {
        Self::ReasoningEffort {
            state: ReasoningEffortState::new(current),
        }
    }

    pub fn command_palette(items: Vec<CommandPaletteItem>) -> Self {
        let filtered_indices = (0..items.len()).collect();
        Self::CommandPalette {
            query: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            items,
            filtered_indices,
        }
    }

    pub fn default_command_palette_items() -> Vec<CommandPaletteItem> {
        vec![
            CommandPaletteItem::new(
                "nav_chat",
                "Switch to Chat Screen (1)",
                "Interactive autonomous conversation and code editing",
                "Navigation",
                Some("Alt+1 / F1"),
            ),
            CommandPaletteItem::new(
                "nav_sessions",
                "Switch to Sessions Screen (2)",
                "Search, manage, resume, and inspect session threads",
                "Navigation",
                Some("Alt+2 / F2"),
            ),
            CommandPaletteItem::new(
                "nav_models",
                "Switch to Models Catalog (3)",
                "Search and activate LLM models across providers",
                "Navigation",
                Some("Alt+3 / F3"),
            ),
            CommandPaletteItem::new(
                "nav_providers",
                "Switch to Providers Screen (4)",
                "Configure endpoints, credentials, and test connectivity",
                "Navigation",
                Some("Alt+4 / F4"),
            ),
            CommandPaletteItem::new(
                "nav_agents",
                "Switch to Agents Screen (5)",
                "Inspect child agent tree, status, and supervise execution",
                "Navigation",
                Some("Alt+5 / F5"),
            ),
            CommandPaletteItem::new(
                "nav_workspace",
                "Switch to Workspace Screen (6)",
                "Browse project structure, trust status, and context",
                "Navigation",
                Some("Alt+6 / F6"),
            ),
            CommandPaletteItem::new(
                "nav_source_control",
                "Switch to Source Control Screen (7)",
                "Review git status, branches, staged changes, and diffs",
                "Navigation",
                Some("Alt+7 / F7"),
            ),
            CommandPaletteItem::new(
                "nav_tools",
                "Switch to Tools & Integrations Screen (8)",
                "Manage MCP servers, built-in tools, and permissions",
                "Navigation",
                Some("Alt+8 / F8"),
            ),
            CommandPaletteItem::new(
                "nav_settings",
                "Switch to Settings Screen (9)",
                "Configure security, git armor, sound, and editor options",
                "Navigation",
                Some("Alt+9 / F9"),
            ),
            CommandPaletteItem::new(
                "nav_diagnostics",
                "Switch to Diagnostics Screen (10)",
                "System healthcheck, supervisor processes, and memory",
                "Navigation",
                Some("Alt+0 / F10"),
            ),
            CommandPaletteItem::new(
                "toggle_inspector",
                "Toggle Context Inspector Pane",
                "Show/hide collapsible contextual inspector sidebar",
                "View",
                Some("Ctrl+B / F12"),
            ),
            CommandPaletteItem::new(
                "settings",
                "Open Settings Screen",
                "Configure models, security policies, theme, and agent preferences",
                "Settings",
                Some("/settings"),
            ),
            CommandPaletteItem::new(
                "mode_toggle",
                "Toggle Agent Mode (Build / Plan)",
                "Switch between autonomous Build mode and advisory Plan mode",
                "Mode",
                Some("Tab"),
            ),
            CommandPaletteItem::new(
                "model_picker",
                "Switch Active Model",
                "Select LLM architecture and provider endpoint",
                "Model",
                Some("Ctrl+M / F3"),
            ),
            CommandPaletteItem::new(
                "mcp_hub",
                "MCP Servers Hub",
                "Browse, inspect, and toggle Model Context Protocol servers & tools",
                "Tools",
                Some("/mcp"),
            ),
            CommandPaletteItem::new(
                "skills_library",
                "Skills Library",
                "Browse discovered workspace & user-global skills and inspect docs",
                "Skills",
                Some("/skills"),
            ),
            CommandPaletteItem::new(
                "templates",
                "Collaboration Templates",
                "Browse preset pair programming, review, and debug templates",
                "Session",
                Some("/template"),
            ),
            CommandPaletteItem::new(
                "goal_dashboard",
                "Goal & Mission Dashboard",
                "Inspect long-running goal progress, token spend, and controls",
                "Mission",
                Some("/goal"),
            ),
            CommandPaletteItem::new(
                "subagents",
                "Subagents Hierarchy",
                "View spawned child agents, swarm lineage, and execution status",
                "Agents",
                Some("/agents"),
            ),
            CommandPaletteItem::new(
                "context_mgr",
                "Context & Pinned Files",
                "Inspect token consumption, pinned files, and world state",
                "Context",
                Some("/context"),
            ),
            CommandPaletteItem::new(
                "persona_picker",
                "Engineering Persona",
                "Switch between Architect, Coder, Auditor, and TDD workstyles",
                "Agent",
                Some("/persona"),
            ),
            CommandPaletteItem::new(
                "git_diff",
                "Interactive Git Diff",
                "Review uncommitted changes and working tree diff",
                "Git",
                Some("Ctrl+D"),
            ),
            CommandPaletteItem::new(
                "resume_picker",
                "Resume Past Session",
                "Browse and switch previous SQLite conversation sessions",
                "Session",
                Some("Ctrl+R"),
            ),
            CommandPaletteItem::new(
                "undo",
                "Undo Last Changes",
                "Revert session and workspace mutations to previous checkpoint",
                "Git",
                Some("/undo"),
            ),
            CommandPaletteItem::new(
                "verify",
                "Run Workspace Verification",
                "Execute autonomous compiler and linter checks",
                "Action",
                Some("/verify"),
            ),
            CommandPaletteItem::new(
                "test",
                "Run Test Suite",
                "Run cargo or workspace tests with diagnostic reporting",
                "Action",
                Some("/test"),
            ),
            CommandPaletteItem::new(
                "review",
                "Automated Code Review",
                "Review git diff with autonomous Guardian supervisor",
                "Action",
                Some("/review"),
            ),
            CommandPaletteItem::new(
                "analytics",
                "Token & Cost Analytics",
                "Inspect session token metrics, model context, and pricing",
                "Metrics",
                Some("Ctrl+A"),
            ),
            CommandPaletteItem::new(
                "checkpoints",
                "Session Checkpoints DAG",
                "Inspect branch tree, save points, and rollback DAG",
                "Session",
                Some("/checkpoints"),
            ),
            CommandPaletteItem::new(
                "procs",
                "Process Supervisor",
                "Inspect and manage background processes and servers",
                "Tools",
                Some("/procs"),
            ),
            CommandPaletteItem::new(
                "locks",
                "Active Locks & Leases",
                "Inspect path-based execution locks in the workspace",
                "System",
                Some("Ctrl+E"),
            ),
            CommandPaletteItem::new(
                "timing",
                "Critical Path Timings",
                "Display causal breakdown of LLM vs tool latencies",
                "Metrics",
                Some("Ctrl+K"),
            ),
            CommandPaletteItem::new(
                "symbol_outline",
                "Symbol Outline",
                "Browse code symbols and structures in current file",
                "Code",
                Some("Ctrl+O"),
            ),
            CommandPaletteItem::new(
                "compact",
                "Compact Session Context",
                "Free up LLM context window through smart summarization",
                "Session",
                Some("/compact"),
            ),
            CommandPaletteItem::new(
                "copy_code",
                "Copy Last Code Block",
                "Extract and copy the most recent code block to clipboard",
                "Code",
                Some("Ctrl+Y"),
            ),
            CommandPaletteItem::new(
                "fold_tools",
                "Fold / Unfold Tool Calls",
                "Toggle collapse state of all tool outputs and reasoning",
                "View",
                Some("Ctrl+T"),
            ),
            CommandPaletteItem::new(
                "scratchpad",
                "Code Mode V8 Scratchpad",
                "Interactive multi-tool TypeScript/JavaScript scratchpad and runtime evaluator",
                "CodeMode",
                Some("/scratchpad"),
            ),
            CommandPaletteItem::new(
                "rollout",
                "Rollout Journal & Events",
                "Inspect append-only timeline of turn items, tool outputs, and diagnostics",
                "System",
                Some("/rollout"),
            ),
            CommandPaletteItem::new(
                "clear",
                "Clear Transcript",
                "Clear current transcript screen",
                "View",
                Some("Ctrl+L"),
            ),
            CommandPaletteItem::new(
                "doctor",
                "System Healthcheck (Doctor)",
                "Run runtime diagnostic environment checks",
                "System",
                Some("/doctor"),
            ),
            CommandPaletteItem::new(
                "session_new",
                "New Session",
                "Start a clean development workspace session",
                "Session",
                Some("Ctrl+N / /new"),
            ),
            CommandPaletteItem::new(
                "worktrees",
                "Git Worktrees",
                "List and manage isolated Git worktree workspaces",
                "Git",
                Some("Ctrl+W"),
            ),
            CommandPaletteItem::new(
                "cycle_reasoning",
                "Cycle Reasoning Effort",
                "Adjust LLM thinking budget between low, medium, and high",
                "Model",
                Some("/effort"),
            ),
            CommandPaletteItem::new(
                "test_provider",
                "Test Provider Connectivity",
                "Probe active LLM provider endpoint and check availability",
                "Model",
                Some("/probe"),
            ),
            CommandPaletteItem::new(
                "panel_tab_files",
                "Panel: Workspace Files",
                "Switch side panel to workspace directory and file browser",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_git",
                "Panel: Git Status & Changes",
                "Switch side panel to Git uncommitted changes",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_diff",
                "Panel: Diff Preview",
                "Switch side panel to uncommitted changes diff preview",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_term",
                "Panel: Background Tasks",
                "Switch side panel to background processes and supervised tasks",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_context",
                "Panel: Dynamic Context & Pinned Items",
                "Switch side panel to pinned files and memory digests",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_activity",
                "Panel: Execution Activity & Timings",
                "Switch side panel to tool execution durations and critical path",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_subagents",
                "Panel: Subagents Hierarchy",
                "Switch side panel to child agent tree and swarm posts",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_tab_diagnostics",
                "Panel: Diagnostics & Telemetry",
                "Switch side panel to token metrics, locks, and integrity",
                "Panel",
                Some("[ / ]"),
            ),
            CommandPaletteItem::new(
                "panel_widen",
                "Panel: Widen Width (+5%)",
                "Increase width percentage allocated to contextual workspace panel",
                "Panel",
                Some("Alt+]"),
            ),
            CommandPaletteItem::new(
                "panel_narrow",
                "Panel: Narrow Width (-5%)",
                "Decrease width percentage allocated to contextual workspace panel",
                "Panel",
                Some("Alt+["),
            ),
            CommandPaletteItem::new(
                "help",
                "Help & Keyboard Guide",
                "Show complete keybindings and commands overview",
                "Help",
                Some("?"),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::types::{FunctionCall, ToolCall};

    #[test]
    fn test_pending_approval_suggested_prefix_and_helpers() {
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "exec_command".to_string(),
                arguments: r#"{"command": "cargo test --all"}"#.to_string(),
            },
        };
        let approval = PendingApproval::new(
            tool_call,
            "High risk shell command".to_string(),
            Some("cargo test --all".to_string()),
        );

        assert_eq!(approval.suggested_prefix(), Some("cargo test".to_string()));
        assert_eq!(approval.target_file(), None);
        assert_eq!(approval.diff_text(), None);

        // Test git command
        let git_approval = PendingApproval::new(
            ToolCall {
                id: "call_2".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "exec_command".to_string(),
                    arguments: r#"{"command": "git status"}"#.to_string(),
                },
            },
            "Git execution".to_string(),
            Some("git status".to_string()),
        );
        assert_eq!(
            git_approval.suggested_prefix(),
            Some("git status".to_string())
        );

        // Test npm run command
        let npm_approval = PendingApproval::new(
            ToolCall {
                id: "call_3".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "exec_command".to_string(),
                    arguments: r#"{"command": "npm run build --prod"}"#.to_string(),
                },
            },
            "Build script".to_string(),
            Some("npm run build --prod".to_string()),
        );
        assert_eq!(
            npm_approval.suggested_prefix(),
            Some("npm run build".to_string())
        );
    }

    #[test]
    fn test_pending_approval_patch_and_file_extraction() {
        let patch_call = ToolCall {
            id: "call_4".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "apply_patch".to_string(),
                arguments: r#"{"path": "crates/m31a-tui/src/lib.rs", "patch": "*** Begin Patch\n+fn hello() {}\n*** End Patch"}"#.to_string(),
            },
        };
        let patch_approval =
            PendingApproval::new(patch_call, "File modification".to_string(), None);

        assert_eq!(
            patch_approval.target_file(),
            Some("crates/m31a-tui/src/lib.rs".to_string())
        );
        assert!(
            patch_approval
                .diff_text()
                .unwrap()
                .contains("*** Begin Patch")
        );
    }

    #[test]
    fn test_code_mode_scratchpad_state_editing_and_navigation() {
        let mut sp = CodeModeScratchpadState::new(
            "declare namespace tools {}".to_string(),
            Some("const x = 1;".to_string()),
        );
        assert_eq!(sp.code, "const x = 1;");
        assert_eq!(sp.focus, ScratchpadFocus::Editor);
        assert_eq!(sp.cursor_line, 0);
        assert_eq!(sp.cursor_col, 0);

        // Insert characters
        sp.insert_char('/');
        sp.insert_char('/');
        sp.insert_char(' ');
        assert!(sp.code.starts_with("// const x = 1;"));

        // Newline insertion
        sp.insert_newline();
        assert_eq!(sp.cursor_line, 1);
        assert_eq!(sp.cursor_col, 0);

        // Backspace
        sp.delete_backspace();
        assert_eq!(sp.cursor_line, 0);

        // Modal constructor
        let modal = ModalState::code_mode_scratchpad("dts".to_string(), None);
        assert!(matches!(modal, ModalState::CodeModeScratchpad(_)));
    }

    #[test]
    fn test_rollout_journal_filtering_and_toggle() {
        let events = vec![
            RolloutEventEntry {
                id: 1,
                timestamp: "12:00:00".to_string(),
                category: "PROMPT".to_string(),
                title: "User prompt".to_string(),
                detail: "{}".to_string(),
                tokens: Some(10),
            },
            RolloutEventEntry {
                id: 2,
                timestamp: "12:00:05".to_string(),
                category: "TOOL".to_string(),
                title: "list_dir".to_string(),
                detail: "{}".to_string(),
                tokens: None,
            },
            RolloutEventEntry {
                id: 3,
                timestamp: "12:00:10".to_string(),
                category: "CHECKPOINT".to_string(),
                title: "Checkpoint #1".to_string(),
                detail: "{}".to_string(),
                tokens: None,
            },
        ];

        let mut journal = RolloutJournalState::new(events);
        assert_eq!(journal.filtered_indices().len(), 3);

        // Filter by prompts
        journal.filter = "prompts".to_string();
        assert_eq!(journal.filtered_indices(), vec![0]);

        // Filter by tools
        journal.filter = "tools".to_string();
        assert_eq!(journal.filtered_indices(), vec![1]);

        // Filter by checkpoints
        journal.filter = "checkpoints".to_string();
        assert_eq!(journal.filtered_indices(), vec![2]);

        // Toggle JSON view
        assert!(!journal.view_json);
        journal.view_json = true;
        assert!(journal.view_json);
    }

    #[test]
    fn test_modal_stack_lifecycle() {
        let mut stack = ModalStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);

        stack.push(ModalState::help());
        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());
        assert!(matches!(
            stack.current(),
            Some(ModalState::HelpModal { .. })
        ));

        // Push second modal
        stack.push(ModalState::settings());
        assert_eq!(stack.len(), 2);
        assert!(matches!(
            stack.current(),
            Some(ModalState::SettingsModal { .. })
        ));

        // Pop returns to parent modal
        let popped = stack.pop();
        assert!(matches!(popped, Some(ModalState::SettingsModal { .. })));
        assert_eq!(stack.len(), 1);
        assert!(matches!(
            stack.current(),
            Some(ModalState::HelpModal { .. })
        ));

        // Clear stack
        stack.clear();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_select_picker_filtering_and_navigation() {
        let items = vec![
            SelectPickerItem::new("cmd1", "Run Build", "Build current project", "Build")
                .with_shortcut("b")
                .with_badge("Fast"),
            SelectPickerItem::new("cmd2", "Run Tests", "Execute test suites", "Test")
                .with_shortcut("t")
                .with_detail("cargo test --all"),
            SelectPickerItem::new("cmd3", "Git Commit", "Commit staged files", "Git")
                .with_shortcut("c"),
        ];

        let mut picker = SelectPickerState::new("Actions", items);
        assert_eq!(picker.filtered_indices.len(), 3);
        assert_eq!(picker.selected_item().unwrap().title, "Run Build");

        // Navigate forward
        picker.select_next();
        assert_eq!(picker.selected_item().unwrap().title, "Run Tests");

        // Filter by query "test"
        picker.set_query("test".to_string());
        assert_eq!(picker.filtered_indices.len(), 1);
        assert_eq!(picker.selected_item().unwrap().title, "Run Tests");

        // Backspace clears query
        picker.on_backspace();
        picker.on_backspace();
        picker.on_backspace();
        picker.on_backspace();
        assert_eq!(picker.filtered_indices.len(), 3);
    }
}
