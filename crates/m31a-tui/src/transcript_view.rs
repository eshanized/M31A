//! Formatted viewport renderer for conversational turns, reasoning blocks, and structured outputs.
//!
//! Calculates scroll positions, line wrapping, and renders formatted conversation items within viewport rectangles.

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::transcript::{NoticeLevel, ToolCallEntry, TranscriptItem};

/// Helper to identify read-only discovery tools that should be grouped together.
pub fn is_read_only_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "read_file_range"
            | "list_dir"
            | "dir_list"
            | "file_search"
            | "grep_search"
            | "find_files"
            | "web_search"
            | "fetch_web_page"
            | "symbol_search"
            | "view_outline"
            | "codebase_search"
            | "search_code"
            | "locate_symbol"
    )
}

/// Collapses long command/tool output while preserving the crucial tail lines
/// (where error messages, exit codes, and test assertions are located).
/// Returns `(Option<omitted_count>, tail_lines)`.
pub fn collapse_output_tail(output: &str, max_lines: usize) -> (Option<usize>, Vec<&str>) {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return (None, lines);
    }
    let omitted = lines.len() - max_lines;
    let tail = lines[omitted..].to_vec();
    (Some(omitted), tail)
}

/// Head + tail collapse splitting output into head preview and tail errors.
pub fn collapse_output_head_tail<'a>(
    lines: &'a [&'a str],
    head_count: usize,
    tail_count: usize,
) -> (Option<usize>, &'a [&'a str], &'a [&'a str]) {
    if lines.len() <= head_count + tail_count {
        return (None, lines, &[]);
    }
    let omitted = lines.len() - (head_count + tail_count);
    let head = &lines[..head_count];
    let tail = &lines[lines.len() - tail_count..];
    (Some(omitted), head, tail)
}

/// Extracts the first concise, meaningful sentence or heading from thinking content
/// to use as a 1-line stable header during streaming and in collapsed state.
pub fn extract_reasoning_title(content: &str) -> Option<String> {
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let clean = trimmed
            .trim_start_matches(|c: char| {
                c == '#' || c == '*' || c == '-' || c == '>' || c.is_whitespace()
            })
            .trim();
        if !clean.is_empty() {
            let first_sent = clean.split(". ").next().unwrap_or(clean);
            let trimmed_sent = first_sent.trim_end_matches(['.', ':', ',', ';']).trim();
            let mut title: String = trimmed_sent.chars().take(60).collect();
            if trimmed_sent.chars().count() > 60 {
                title.push_str("...");
            }
            return Some(title);
        }
    }
    None
}

/// Extracts primary target argument (e.g. file path, pattern, or command) from JSON or raw text.
fn extract_primary_arg(args: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(args)
        && let Some(obj) = v.as_object()
    {
        for key in &["path", "query", "pattern", "command", "url", "dir"] {
            if let Some(val) = obj.get(*key).and_then(|v| v.as_str()) {
                let mut s = val.to_string();
                if s.len() > 36 {
                    s.truncate(33);
                    s.push_str("...");
                }
                return s;
            }
        }
    }
    let mut s = args.replace('\n', " ");
    if s.len() > 36 {
        s.truncate(33);
        s.push_str("...");
    }
    s
}

/// Extracted fenced code block from an assistant markdown message.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedCodeBlock {
    pub language: Option<String>,
    pub code: String,
    pub line_count: usize,
}

/// A structured renderable cell in the virtualized transcript.
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptCell {
    User {
        id: String,
        text: String,
        timestamp: DateTime<Utc>,
    },
    Thinking {
        id: String,
        content: String,
        title: Option<String>,
        collapsed: bool,
        duration_ms: Option<u64>,
    },
    Assistant {
        id: String,
        markdown: String,
        code_blocks: Vec<ExtractedCodeBlock>,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
        output: Option<String>,
        success: bool,
        collapsed: bool,
        duration_ms: Option<u64>,
    },
    ExplorationGroup {
        id: String,
        tools: Vec<ToolCallEntry>,
        collapsed: bool,
        in_progress: bool,
    },
    Notice {
        text: String,
        level: NoticeLevel,
    },
}

impl TranscriptCell {
    /// Constructs a User cell.
    pub fn user(id: impl Into<String>, text: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self::User {
            id: id.into(),
            text: text.into(),
            timestamp,
        }
    }

    /// Constructs an Assistant cell and extracts fenced code blocks.
    pub fn assistant(id: impl Into<String>, markdown: impl Into<String>) -> Self {
        let md = markdown.into();
        let code_blocks = extract_code_blocks(&md);
        Self::Assistant {
            id: id.into(),
            markdown: md,
            code_blocks,
        }
    }

    /// Constructs a Thinking cell with automatic title extraction.
    pub fn thinking(
        id: impl Into<String>,
        content: impl Into<String>,
        collapsed: bool,
        duration_ms: Option<u64>,
    ) -> Self {
        let content_str = content.into();
        let title = extract_reasoning_title(&content_str);
        Self::Thinking {
            id: id.into(),
            content: content_str,
            title,
            collapsed,
            duration_ms,
        }
    }

    /// Constructs a ToolCall cell from a ToolCallEntry.
    pub fn tool_call(entry: &ToolCallEntry) -> Self {
        Self::ToolCall {
            id: entry.id.clone(),
            name: entry.name.clone(),
            arguments: entry.arguments.clone(),
            output: entry.output.clone(),
            success: entry.success.unwrap_or(true),
            collapsed: entry.collapsed,
            duration_ms: entry.duration_ms,
        }
    }

    /// Constructs an ExplorationGroup cell.
    pub fn exploration_group(
        id: impl Into<String>,
        tools: Vec<ToolCallEntry>,
        collapsed: bool,
        in_progress: bool,
    ) -> Self {
        Self::ExplorationGroup {
            id: id.into(),
            tools,
            collapsed,
            in_progress,
        }
    }

    /// Toggles collapse status for collapsible cells.
    pub fn toggle_collapsed(&mut self) {
        match self {
            TranscriptCell::Thinking { collapsed, .. } => *collapsed = !*collapsed,
            TranscriptCell::ToolCall { collapsed, .. } => *collapsed = !*collapsed,
            TranscriptCell::ExplorationGroup { collapsed, .. } => *collapsed = !*collapsed,
            _ => {}
        }
    }

    /// Returns whether this cell is currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        match self {
            TranscriptCell::Thinking { collapsed, .. } => *collapsed,
            TranscriptCell::ToolCall { collapsed, .. } => *collapsed,
            TranscriptCell::ExplorationGroup { collapsed, .. } => *collapsed,
            _ => false,
        }
    }
}

/// Helper function to extract fenced code blocks from markdown text with full support for variable
/// fence lengths (```` ````, ```` ````), tildes (`~~~`), and language tags.
pub fn extract_code_blocks(text: &str) -> Vec<ExtractedCodeBlock> {
    let mut blocks = Vec::new();
    let mut current_fence_char: Option<char> = None;
    let mut current_fence_len = 0;
    let mut current_lang = None;
    let mut current_code = String::new();

    for line in text.lines() {
        let trimmed_start = line.trim_start();
        let indent = line.len() - trimmed_start.len();

        if let Some(fence_char) = current_fence_char {
            // Check if this line is a closing fence
            if indent <= 3 {
                let trimmed = trimmed_start.trim();
                let fence_count = trimmed.chars().take_while(|&c| c == fence_char).count();
                if fence_count >= current_fence_len && trimmed[fence_count..].trim().is_empty() {
                    // Valid closing fence
                    let line_count = current_code.lines().count();
                    blocks.push(ExtractedCodeBlock {
                        language: current_lang.take(),
                        code: current_code.trim_end().to_string(),
                        line_count,
                    });
                    current_code.clear();
                    current_fence_char = None;
                    current_fence_len = 0;
                    continue;
                }
            }
            current_code.push_str(line);
            current_code.push('\n');
        } else if indent <= 3 {
            // Check for opening fence: at least 3 backticks or 3 tildes
            if trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~") {
                let fence_char = trimmed_start.chars().next().unwrap();
                let fence_len = trimmed_start
                    .chars()
                    .take_while(|&c| c == fence_char)
                    .count();
                if fence_len >= 3 {
                    let info = trimmed_start[fence_len..].trim();
                    let lang = info.split_whitespace().next().map(|s| s.to_string());
                    current_fence_char = Some(fence_char);
                    current_fence_len = fence_len;
                    current_lang = lang;
                    current_code.clear();
                    continue;
                }
            }
        }
    }

    // Unclosed fence during streaming: extract partial content cleanly
    if current_fence_char.is_some() && !current_code.is_empty() {
        let line_count = current_code.lines().count();
        blocks.push(ExtractedCodeBlock {
            language: current_lang,
            code: current_code.trim_end().to_string(),
            line_count,
        });
    }

    blocks
}

/// Virtualizer managing cells, fast viewport slicing, and cached visual-row metrics.
///
/// Every metric stored in this type is expressed in *visual terminal rows* produced by
/// [`TranscriptVirtualizer::render_cell_visual_rows_static`].  There is deliberately no
/// second logical-line measurement path: if a caller needs a height, it asks this type,
/// and the number it receives is exactly the number of rows the renderer will emit.
#[derive(Debug, Clone, Default)]
pub struct TranscriptVirtualizer {
    cells: Vec<TranscriptCell>,
    pub expanded_ids: HashSet<String>,
    cache_width: Cell<u16>,
    cached_line_counts: RefCell<Vec<usize>>,
    cached_total_lines: Cell<Option<usize>>,
}

impl TranscriptVirtualizer {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            expanded_ids: HashSet::new(),
            cache_width: Cell::new(0),
            cached_line_counts: RefCell::new(Vec::new()),
            cached_total_lines: Cell::new(None),
        }
    }

    /// Returns the cells currently held by the virtualizer.
    pub fn cells(&self) -> &[TranscriptCell] {
        &self.cells
    }

    /// Replaces the entire cell list and invalidates all cached visual-row metrics.
    pub fn set_cells(&mut self, cells: Vec<TranscriptCell>) {
        self.cells = cells;
        self.invalidate_cache();
    }

    /// Toggles every collapsible cell and invalidates the affected metrics.
    pub fn toggle_all_collapsed(&mut self) {
        for cell in &mut self.cells {
            cell.toggle_collapsed();
        }
        self.invalidate_cache();
    }

    /// Toggles a single cell by index and invalidates only that cell's cached height.
    ///
    /// Returns `false` when the index is out of range.
    pub fn toggle_cell(&mut self, index: usize) -> bool {
        let Some(cell) = self.cells.get_mut(index) else {
            return false;
        };
        cell.toggle_collapsed();
        self.invalidate_cell_metrics(index);
        true
    }

    /// Invalidates cached line count metrics when transcript items or terminal geometry change.
    pub fn invalidate_cache(&self) {
        self.cached_total_lines.set(None);
        self.cached_line_counts.borrow_mut().clear();
    }

    /// Recomputes the cached height for a single cell without discarding the rest of the cache.
    fn invalidate_cell_metrics(&self, index: usize) {
        let width = self.cache_width.get();
        if width == 0 {
            self.invalidate_cache();
            return;
        }
        let mut counts = self.cached_line_counts.borrow_mut();
        if counts.len() != self.cells.len() {
            drop(counts);
            self.invalidate_cache();
            return;
        }
        let Some(cell) = self.cells.get(index) else {
            drop(counts);
            self.invalidate_cache();
            return;
        };
        let old = counts[index];
        let new = Self::measure_cell_rendered_height(cell, width);
        if old != new {
            counts[index] = new;
            let total = self.cached_total_lines.get().unwrap_or(0);
            self.cached_total_lines
                .set(Some(total.saturating_sub(old).saturating_add(new)));
        }
    }

    /// Accurately measures the rendered terminal rows for a cell at the given column width.
    ///
    /// This is the single measurement contract of the transcript: it is defined as the
    /// length of the canonical visual-row sequence used by [`Self::render_virtualized`].
    pub fn measure_cell_rendered_height(cell: &TranscriptCell, width: u16) -> usize {
        Self::render_cell_visual_rows_static(cell, width).len()
    }

    /// Ensures cached visual-row counts are populated for the specified terminal column width.
    fn ensure_cache(&self, width: u16) {
        let cache_valid = self.cache_width.get() == width
            && self.cached_total_lines.get().is_some()
            && self.cached_line_counts.borrow().len() == self.cells.len();
        if cache_valid {
            return;
        }
        let mut counts = Vec::with_capacity(self.cells.len());
        let mut total = 0;
        for cell in &self.cells {
            let count = Self::measure_cell_rendered_height(cell, width);
            counts.push(count);
            total += count;
        }
        self.cache_width.set(width);
        *self.cached_line_counts.borrow_mut() = counts;
        self.cached_total_lines.set(Some(total));
    }

    /// Converts standard TranscriptItems into structured cells, grouping consecutive read-only
    /// discovery operations into unified ExplorationGroup cells.
    pub fn from_transcript_items(items: &[TranscriptItem]) -> Self {
        Self::from_transcript_items_preserving_state(items, &HashSet::new())
    }

    /// Converts standard TranscriptItems into structured cells while preserving user-expanded group states.
    pub fn from_transcript_items_preserving_state(
        items: &[TranscriptItem],
        expanded_ids: &HashSet<String>,
    ) -> Self {
        let mut cells = Vec::new();
        let mut i = 0;

        while i < items.len() {
            match &items[i] {
                TranscriptItem::ToolCall(entry) if is_read_only_tool(&entry.name) => {
                    let mut group_tools = vec![entry.clone()];
                    let mut j = i + 1;
                    while j < items.len() {
                        if let TranscriptItem::ToolCall(next_entry) = &items[j]
                            && is_read_only_tool(&next_entry.name)
                        {
                            group_tools.push(next_entry.clone());
                            j += 1;
                            continue;
                        }
                        break;
                    }

                    if group_tools.len() > 1 {
                        let id = format!("cell-group-{i}");
                        let in_progress = group_tools.iter().any(|t| t.in_progress);
                        let collapsed = !expanded_ids.contains(&id);
                        cells.push(TranscriptCell::ExplorationGroup {
                            id,
                            tools: group_tools,
                            collapsed,
                            in_progress,
                        });
                        i = j;
                    } else {
                        cells.push(TranscriptCell::tool_call(entry));
                        i += 1;
                    }
                }
                TranscriptItem::User { text, timestamp } => {
                    let id = format!("cell-{i}");
                    cells.push(TranscriptCell::user(id, text, *timestamp));
                    i += 1;
                }
                TranscriptItem::Thinking { text, collapsed } => {
                    let id = format!("cell-{i}");
                    cells.push(TranscriptCell::thinking(id, text, *collapsed, None));
                    i += 1;
                }
                TranscriptItem::Assistant { text } => {
                    let id = format!("cell-{i}");
                    cells.push(TranscriptCell::assistant(id, text));
                    i += 1;
                }
                TranscriptItem::ToolCall(entry) => {
                    cells.push(TranscriptCell::tool_call(entry));
                    i += 1;
                }
                TranscriptItem::Notice { text, level } => {
                    cells.push(TranscriptCell::Notice {
                        text: text.clone(),
                        level: level.clone(),
                    });
                    i += 1;
                }
            }
        }

        Self {
            cells,
            expanded_ids: expanded_ids.clone(),
            cache_width: Cell::new(0),
            cached_line_counts: RefCell::new(Vec::new()),
            cached_total_lines: Cell::new(None),
        }
    }

    /// Toggles exploration group expanded state and records it in `expanded_ids`.
    pub fn toggle_exploration_group(&mut self, cell_id: &str) {
        for cell in &mut self.cells {
            if let TranscriptCell::ExplorationGroup { id, collapsed, .. } = cell
                && id == cell_id
            {
                *collapsed = !*collapsed;
                if !*collapsed {
                    self.expanded_ids.insert(id.clone());
                } else {
                    self.expanded_ids.remove(id);
                }
                self.invalidate_cache();
                break;
            }
        }
    }

    /// Finds the last code block in the transcript for quick copying (e.g. Ctrl+Y).
    pub fn find_last_code_block(&self) -> Option<&ExtractedCodeBlock> {
        for cell in self.cells.iter().rev() {
            if let TranscriptCell::Assistant { code_blocks, .. } = cell
                && let Some(last) = code_blocks.last()
            {
                return Some(last);
            }
        }
        None
    }

    /// Builds the logical (unwrapped) lines for a cell.
    ///
    /// This function is deliberately private: wrapping and measurement must only ever go
    /// through [`Self::render_cell_visual_rows_static`], which is the canonical projection
    /// of a cell onto terminal rows.
    fn render_cell_logical_lines(cell: &TranscriptCell, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let max_w = (width as usize).saturating_sub(4).max(10);

        match cell {
            TranscriptCell::User {
                text, timestamp, ..
            } => {
                let time_str = timestamp.format("%H:%M:%S").to_string();
                lines.push(Line::from(vec![
                    Span::styled(
                        "◈ You ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({time_str})"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));

                for l in text.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(l.to_string(), Style::default().fg(Color::White)),
                    ]));
                }
                lines.push(Line::raw(""));
            }

            TranscriptCell::Thinking {
                content,
                title,
                collapsed,
                duration_ms,
                ..
            } => {
                let dur_str = duration_ms
                    .map(|ms| format!(" ({:.1}s)", ms as f64 / 1000.0))
                    .unwrap_or_default();
                let token_approx = (content.len() / 4).max(1);
                let badge = format!(" (~{} tok{})", token_approx, dur_str);
                let title_display = title.as_deref().unwrap_or("Thinking");

                if *collapsed {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("▶ ● Progress: {}", title_display),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::styled(badge, Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            " [Space to view execution step]",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("▼ ● Progress: {}", title_display),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(badge, Style::default().fg(Color::DarkGray)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  │ ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            format!("Execution Step: {}", title_display),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                lines.push(Line::raw(""));
            }

            TranscriptCell::Assistant { markdown, .. } => {
                lines.push(Line::from(vec![Span::styled(
                    "◆ M31A",
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )]));

                let rendered_md = crate::markdown_syntax::render_markdown_to_lines(markdown);
                lines.extend(rendered_md);
                lines.push(Line::raw(""));
            }

            TranscriptCell::ExplorationGroup {
                tools,
                collapsed,
                in_progress,
                ..
            } => {
                let total_tools = tools.len();
                let mut read_count = 0;
                let mut search_count = 0;
                let mut dir_count = 0;
                let mut other_count = 0;

                for t in tools {
                    if t.name.contains("read") {
                        read_count += 1;
                    } else if t.name.contains("search")
                        || t.name.contains("grep")
                        || t.name.contains("find")
                    {
                        search_count += 1;
                    } else if t.name.contains("dir") || t.name.contains("list") {
                        dir_count += 1;
                    } else {
                        other_count += 1;
                    }
                }

                let mut parts = Vec::new();
                if read_count > 0 {
                    parts.push(format!(
                        "{} read{}",
                        read_count,
                        if read_count > 1 { "s" } else { "" }
                    ));
                }
                if search_count > 0 {
                    parts.push(format!(
                        "{} search{}",
                        search_count,
                        if search_count > 1 { "es" } else { "" }
                    ));
                }
                if dir_count > 0 {
                    parts.push(format!(
                        "{} listing{}",
                        dir_count,
                        if dir_count > 1 { "s" } else { "" }
                    ));
                }
                if other_count > 0 {
                    parts.push(format!("{} other", other_count));
                }
                let breakdown = parts.join(", ");

                if *collapsed {
                    let status_badge = if *in_progress { " (running...)" } else { "" };
                    lines.push(Line::from(vec![
                        Span::styled(
                            "▶ ⚲ Explored: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{} tools (", total_tools),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(breakdown, Style::default().fg(Color::LightCyan)),
                        Span::styled(
                            format!("){}", status_badge),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            " [collapsed, click or Space to expand]",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![Span::styled(
                        format!("▼ ⚲ Explored ({} discovery tools)", total_tools),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )]));

                    for tool in tools {
                        let (status_icon, status_color) = match tool.success {
                            Some(true) => ("✔", Color::Green),
                            Some(false) => ("✖", Color::Red),
                            None => {
                                if tool.in_progress {
                                    ("◷", Color::Yellow)
                                } else {
                                    ("✔", Color::Green)
                                }
                            }
                        };

                        let dur_str = tool
                            .duration_ms
                            .map(|ms| format!(" ({}ms)", ms))
                            .unwrap_or_default();

                        let target_summary = extract_primary_arg(&tool.arguments);

                        let lines_info = if let Some(out) = &tool.output {
                            format!(", {} lines", out.lines().count())
                        } else {
                            String::new()
                        };

                        lines.push(Line::from(vec![
                            Span::styled("  │ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("{:<14} ", tool.name),
                                Style::default().fg(Color::LightBlue),
                            ),
                            Span::styled(target_summary, Style::default().fg(Color::White)),
                            Span::styled(" [", Style::default().fg(Color::DarkGray)),
                            Span::styled(status_icon, Style::default().fg(status_color)),
                            Span::styled(
                                format!("{}{}]", dur_str, lines_info),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }
                lines.push(Line::raw(""));
            }

            TranscriptCell::ToolCall {
                name,
                arguments,
                output,
                success,
                collapsed,
                duration_ms,
                ..
            } => {
                let status_icon = if *success { "✔ OK" } else { "✖ FAIL" };
                let status_color = if *success { Color::Green } else { Color::Red };
                let dur_str = duration_ms
                    .map(|ms| format!(" ({}ms)", ms))
                    .unwrap_or_default();

                let out_line_count = output.as_ref().map(|o| o.lines().count()).unwrap_or(0);

                // Determine tool category icon & style
                let (tool_icon, tool_badge_color) = if name.contains("patch")
                    || name.contains("write")
                    || name.contains("edit")
                {
                    ("⟨/⟩", Color::Cyan)
                } else if name.contains("read") || name.contains("view") || name.contains("list") {
                    ("≡", Color::Blue)
                } else if name.contains("exec")
                    || name.contains("shell")
                    || name == "sh"
                    || name == "bash"
                {
                    ("❯_", Color::Yellow)
                } else if name.contains("search") || name.contains("web") || name.contains("fetch")
                {
                    ("⚲", Color::LightMagenta)
                } else if name.contains("agent")
                    || name.contains("spawn")
                    || name.contains("subagent")
                {
                    ("◇", Color::Green)
                } else if name.contains("goal") {
                    ("✓", Color::LightYellow)
                } else {
                    ("⚙", Color::White)
                };

                if *collapsed {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("▶ tool_call {} ", name),
                            Style::default()
                                .fg(tool_badge_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{} ", tool_icon),
                            Style::default().fg(tool_badge_color),
                        ),
                        Span::styled(
                            format!("-> {} lines{}", out_line_count, dur_str),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            " [collapsed, Space/click to expand]",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("▼ tool_call {} ", name),
                            Style::default()
                                .fg(tool_badge_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{} ", tool_icon),
                            Style::default().fg(tool_badge_color),
                        ),
                        Span::styled(
                            format!("[{}{}]", status_icon, dur_str),
                            Style::default()
                                .fg(status_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    let args_preview = if arguments.len() > max_w {
                        format!("{}...", &arguments[..max_w])
                    } else {
                        arguments.clone()
                    };
                    lines.push(Line::from(vec![
                        Span::styled("    args: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(args_preview, Style::default().fg(Color::White)),
                    ]));

                    if let Some(out) = output {
                        let out_lines: Vec<&str> = out.lines().collect();

                        // Check if output is a unified diff or patch
                        if out.starts_with("---")
                            || out.contains("@@ -")
                            || out.starts_with("*** Begin Patch")
                        {
                            if out_lines.len() <= 100 {
                                for l in &out_lines {
                                    let style = if l.starts_with('+') && !l.starts_with("+++") {
                                        Style::default().fg(Color::Green)
                                    } else if l.starts_with('-') && !l.starts_with("---") {
                                        Style::default().fg(Color::Red)
                                    } else if l.starts_with("@@") {
                                        Style::default().fg(Color::Cyan)
                                    } else {
                                        Style::default().fg(Color::DarkGray)
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            "    │ ",
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(l.to_string(), style),
                                    ]));
                                }
                            } else {
                                let (omitted, head, tail) =
                                    collapse_output_head_tail(&out_lines, 20, 80);
                                for l in head {
                                    let style = if l.starts_with('+') && !l.starts_with("+++") {
                                        Style::default().fg(Color::Green)
                                    } else if l.starts_with('-') && !l.starts_with("---") {
                                        Style::default().fg(Color::Red)
                                    } else if l.starts_with("@@") {
                                        Style::default().fg(Color::Cyan)
                                    } else {
                                        Style::default().fg(Color::DarkGray)
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            "    │ ",
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(l.to_string(), style),
                                    ]));
                                }
                                if let Some(omitted_count) = omitted {
                                    lines.push(Line::from(vec![
                                        Span::styled(format!("    ... omitted {} lines (press Space to expand full diff) ...", omitted_count), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                                    ]));
                                }
                                for l in tail {
                                    let style = if l.starts_with('+') && !l.starts_with("+++") {
                                        Style::default().fg(Color::Green)
                                    } else if l.starts_with('-') && !l.starts_with("---") {
                                        Style::default().fg(Color::Red)
                                    } else if l.starts_with("@@") {
                                        Style::default().fg(Color::Cyan)
                                    } else {
                                        Style::default().fg(Color::DarkGray)
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            "    │ ",
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(l.to_string(), style),
                                    ]));
                                }
                            }
                        } else if out_lines.len() <= 100 {
                            for (line_idx, l) in out_lines.iter().enumerate() {
                                let prefix = format!("{:4} │ ", line_idx + 1);
                                lines.push(Line::from(vec![
                                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                                    Span::styled(
                                        l.to_string(),
                                        Style::default().fg(Color::LightCyan),
                                    ),
                                ]));
                            }
                        } else {
                            let (omitted, head, tail) =
                                collapse_output_head_tail(&out_lines, 20, 80);
                            for (line_idx, l) in head.iter().enumerate() {
                                let prefix = format!("{:4} │ ", line_idx + 1);
                                lines.push(Line::from(vec![
                                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                                    Span::styled(
                                        l.to_string(),
                                        Style::default().fg(Color::LightCyan),
                                    ),
                                ]));
                            }
                            if let Some(omitted_count) = omitted {
                                lines.push(Line::from(vec![
                                    Span::styled(format!("    ... omitted {} lines (press Space to expand full output) ...", omitted_count), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
                                ]));
                            }
                            let tail_start = out_lines.len().saturating_sub(tail.len());
                            for (tail_idx, l) in tail.iter().enumerate() {
                                let prefix = format!("{:4} │ ", tail_start + tail_idx + 1);
                                lines.push(Line::from(vec![
                                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                                    Span::styled(
                                        l.to_string(),
                                        Style::default().fg(Color::LightCyan),
                                    ),
                                ]));
                            }
                        }
                    }
                }
                lines.push(Line::raw(""));
            }

            TranscriptCell::Notice { text, level } => {
                let (prefix, color) = match level {
                    NoticeLevel::Info => ("ℹ Info: ", Color::LightCyan),
                    NoticeLevel::Warning => ("⚠ Warning: ", Color::Yellow),
                    NoticeLevel::Error => ("✖ Error: ", Color::Red),
                    NoticeLevel::Success => ("✔ Success: ", Color::Green),
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(text.clone(), Style::default().fg(color)),
                ]));
                lines.push(Line::raw(""));
            }
        }

        lines
    }

    /// Computes total rendered line count across all cells for a given width using cached metrics.
    pub fn total_line_count(&self, width: u16) -> usize {
        self.ensure_cache(width);
        self.cached_total_lines.get().unwrap_or(0)
    }

    /// Calculates the visible window of cell indices for virtualized rendering.
    /// Returns `(start_cell_idx, end_cell_idx)`.
    pub fn calculate_visible_window(
        &self,
        scroll_offset: usize,
        viewport_height: usize,
        width: u16,
    ) -> (usize, usize) {
        if self.cells.is_empty() {
            return (0, 0);
        }
        self.ensure_cache(width);
        let counts = self.cached_line_counts.borrow();

        let mut current_line = 0;
        let mut start_idx = 0;
        let mut end_idx = 0;
        let mut found_start = false;

        let end_line = scroll_offset + viewport_height;

        for (idx, &cell_lines) in counts.iter().enumerate() {
            let next_line = current_line + cell_lines;

            if !found_start && next_line > scroll_offset {
                start_idx = idx;
                found_start = true;
            }

            if next_line >= end_line {
                end_idx = idx + 1;
                break;
            }

            current_line = next_line;
            end_idx = idx + 1;
        }

        (start_idx, end_idx.min(self.cells.len()))
    }

    /// Renders the virtualized transcript into the frame area with vertical scroll offset.
    pub fn render_virtualized(&self, f: &mut Frame, area: Rect, scroll_offset: usize) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        self.ensure_cache(area.width);
        let (start_idx, end_idx) =
            self.calculate_visible_window(scroll_offset, area.height as usize, area.width);

        let counts = self.cached_line_counts.borrow();
        let accumulated_lines_before_start: usize = counts.iter().take(start_idx).sum();
        drop(counts);

        let mut all_visible_rows = Vec::new();
        for cell in self.cells.iter().take(end_idx).skip(start_idx) {
            all_visible_rows.extend(Self::render_cell_visual_rows_static(cell, area.width));
        }

        // Sub-cell vertical offset in visual terminal rows
        let sub_offset = scroll_offset.saturating_sub(accumulated_lines_before_start);
        let clipped_rows: Vec<Line<'static>> = all_visible_rows
            .into_iter()
            .skip(sub_offset)
            .take(area.height as usize)
            .collect();

        let paragraph = Paragraph::new(clipped_rows);
        f.render_widget(paragraph, area);
    }

    /// Retrieves the plain text for a specific rendered line index.
    pub fn get_line_as_string(&self, line_index: usize, width: u16) -> Option<String> {
        self.ensure_cache(width);
        let counts = self.cached_line_counts.borrow().clone();
        let mut current = 0;
        for (idx, count) in counts.iter().enumerate() {
            if current + count > line_index {
                let offset = line_index - current;
                let rows = Self::render_cell_visual_rows_static(&self.cells[idx], width);
                if let Some(row) = rows.get(offset) {
                    let text: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
                    return Some(text);
                }
                return None;
            }
            current += count;
        }
        None
    }

    /// Canonical projection of a cell onto terminal rows at a given column width.
    ///
    /// The returned vector has exactly the rows that [`Self::render_virtualized`] writes to
    /// the terminal. Every height, offset, cache and hit-test in this module derives from
    /// this one function, so measurement and rendering cannot disagree.
    pub fn render_cell_visual_rows_static(cell: &TranscriptCell, width: u16) -> Vec<Line<'static>> {
        let logical_lines = Self::render_cell_logical_lines(cell, width);
        let max_w = (width as usize).max(1);
        let mut visual_rows = Vec::new();
        for line in logical_lines {
            visual_rows.extend(wrap_line_to_visual_rows(line, max_w));
        }
        visual_rows
    }

    /// Finds the cell and local line offset containing the specified line index.
    pub fn find_cell_and_local_line(
        &mut self,
        line_index: usize,
        width: u16,
    ) -> Option<(&mut TranscriptCell, usize)> {
        self.ensure_cache(width);
        let counts = self.cached_line_counts.borrow().clone();
        let mut current = 0;
        for (idx, count) in counts.iter().enumerate() {
            if current + count > line_index {
                let local_idx = line_index - current;
                return Some((&mut self.cells[idx], local_idx));
            }
            current += count;
        }
        None
    }

    /// Toggles the collapsible cell whose first visual row contains `line_index`.
    ///
    /// Only the header row of a collapsible cell is interactive, matching what the
    /// renderer displays. Returns `true` when a cell was toggled.
    pub fn toggle_cell_at_line(&mut self, line_index: usize, width: u16) -> bool {
        self.ensure_cache(width);
        let counts = self.cached_line_counts.borrow().clone();
        let mut current = 0;
        for (idx, count) in counts.iter().enumerate() {
            if current + count > line_index {
                if line_index != current {
                    return false;
                }
                let collapsible = matches!(
                    self.cells.get(idx),
                    Some(TranscriptCell::ToolCall { .. })
                        | Some(TranscriptCell::Thinking { .. })
                        | Some(TranscriptCell::ExplorationGroup { .. })
                );
                return collapsible && self.toggle_cell(idx);
            }
            current += count;
        }
        false
    }
}

/// One display glyph plus the style it must retain across wrapping.
#[derive(Debug, Clone)]
struct WrappedGlyph {
    text: String,
    style: Style,
    width: usize,
    is_whitespace: bool,
}

impl WrappedGlyph {
    fn new(text: &str, style: Style) -> Self {
        Self {
            text: text.to_string(),
            style,
            width: UnicodeWidthStr::width(text),
            is_whitespace: !text.is_empty() && text.chars().all(char::is_whitespace),
        }
    }
}

fn build_wrapped_row(glyphs: Vec<WrappedGlyph>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for glyph in glyphs {
        if let Some(last) = spans.last_mut()
            && last.style == glyph.style
        {
            last.content.to_mut().push_str(&glyph.text);
            continue;
        }
        spans.push(Span::styled(glyph.text, glyph.style));
    }
    Line::from(spans)
}

/// Wraps a logical [`Line`] into visual terminal rows of at most `max_width` cells.
///
/// Wrapping operates on Unicode grapheme clusters, so combining marks, zero-width
/// joiners, emoji sequences and wide CJK glyphs are never split. Same-style graphemes
/// are merged back into spans, preserving the caller's styling. A grapheme that is
/// itself wider than `max_width` is emitted alone on its own row because no narrower
/// representation exists.
pub fn wrap_line_to_visual_rows(line: Line<'static>, max_width: usize) -> Vec<Line<'static>> {
    let max_w = max_width.max(1);
    let total_width: usize = line
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    if total_width <= max_w {
        return vec![line];
    }

    let mut glyphs: Vec<WrappedGlyph> = Vec::new();
    for span in line.spans {
        let style = span.style;
        for grapheme in span.content.graphemes(true) {
            glyphs.push(WrappedGlyph::new(grapheme, style));
        }
    }

    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut row: Vec<WrappedGlyph> = Vec::new();
    let mut row_width = 0usize;

    for glyph in glyphs {
        loop {
            let fits = row_width + glyph.width <= max_w;
            let oversized_alone = row.is_empty() && glyph.width > max_w;
            if fits || oversized_alone {
                row_width += glyph.width;
                row.push(glyph);
                break;
            }

            if row.is_empty() {
                // Defensive: a zero-width glyph always fits, so this is unreachable in
                // practice. Placing it keeps the loop total rather than dropping content.
                row_width += glyph.width;
                row.push(glyph);
                break;
            }

            // A separator that does not fit is the end of the current row: flush and
            // drop it rather than starting the next row with a leading space.
            if glyph.is_whitespace {
                rows.push(build_wrapped_row(std::mem::take(&mut row)));
                row_width = 0;
                break;
            }

            // Prefer a soft break at the last whitespace run so words stay intact.
            if let Some(space_index) = row.iter().rposition(|g| g.is_whitespace) {
                let mut run_end = space_index + 1;
                while run_end < row.len() && row[run_end].is_whitespace {
                    run_end += 1;
                }
                let carry = row.split_off(run_end);
                while row.last().is_some_and(|g| g.is_whitespace) {
                    row.pop();
                }
                if !row.is_empty() {
                    rows.push(build_wrapped_row(std::mem::take(&mut row)));
                }
                row = carry;
                row_width = row.iter().map(|g| g.width).sum();
                continue;
            }

            // No break opportunity: hard-wrap at the current position.
            rows.push(build_wrapped_row(std::mem::take(&mut row)));
            row_width = 0;
        }
    }

    rows.push(build_wrapped_row(row));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn backend_rows(
        virt: &TranscriptVirtualizer,
        width: u16,
        height: u16,
        scroll_offset: usize,
    ) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| virt.render_virtualized(f, f.area(), scroll_offset))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                let mut text = String::new();
                let mut x = 0u16;
                while x < width {
                    let symbol = buffer[(x, y)].symbol();
                    text.push_str(symbol);
                    // Wide glyphs occupy the following cell as a blank continuation;
                    // skip it so the reconstruction matches the canonical span text.
                    x += UnicodeWidthStr::width(symbol).max(1) as u16;
                }
                text.trim_end().to_string()
            })
            .collect()
    }

    fn canonical_rows(virt: &TranscriptVirtualizer, width: u16) -> Vec<String> {
        virt.cells()
            .iter()
            .flat_map(|cell| TranscriptVirtualizer::render_cell_visual_rows_static(cell, width))
            .map(|line| line.to_string().trim_end().to_string())
            .collect()
    }

    #[test]
    fn canonical_visual_rows_match_rendered_buffer_at_every_width() {
        let cells = vec![
            TranscriptCell::user(
                "cell-user",
                "你好世界 👨‍👩‍👧‍👦 e\u{301}e\u{301} and a long tail of text",
                Utc::now(),
            ),
            TranscriptCell::thinking(
                "cell-thinking",
                "Reasoning that is long enough to wrap at narrow widths and includes 你好 and emoji 🚀🚀 and combining e\u{301}.",
                false,
                Some(1200),
            ),
            TranscriptCell::assistant(
                "cell-assistant",
                "An assistant answer with a very long paragraph that must wrap across many terminal rows without dropping glyphs 你好世界 🚀 and with `inline_code()` plus a fenced block:\n\n```rust\nfn main() { println!(\"hello world from a fairly long code line\"); }\n```\n",
            ),
            TranscriptCell::tool_call(&ToolCallEntry {
                id: "call-1".into(),
                name: "exec_command".into(),
                arguments: "{\"command\":\"printf '你好世界 %.0s' {1..40}\"}".into(),
                output: Some(
                    (0..120)
                        .map(|i| format!("line {i}: 你好世界 🚀 a fairly long output line"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                success: Some(true),
                in_progress: false,
                collapsed: false,
                duration_ms: Some(42),
            }),
            TranscriptCell::Notice {
                text:
                    "A notice with an intentionally long body that wraps at narrow widths 你好 🚀"
                        .into(),
                level: NoticeLevel::Warning,
            },
        ];

        for width in [1u16, 2, 5, 10, 20, 40, 60, 80, 100, 120, 160] {
            for cell in &cells {
                let rows = TranscriptVirtualizer::render_cell_visual_rows_static(cell, width);
                assert!(!rows.is_empty(), "cell must render at least one row");
                assert_eq!(
                    TranscriptVirtualizer::measure_cell_rendered_height(cell, width),
                    rows.len(),
                    "measurement diverged from canonical rows at width {width}"
                );
                for row in &rows {
                    if width > 1 {
                        assert!(
                            row.width() <= width as usize,
                            "row wider than {width}: {:?} (width {})",
                            row.to_string(),
                            row.width()
                        );
                    }
                }

                let mut virt = TranscriptVirtualizer::new();
                virt.set_cells(vec![cell.clone()]);
                assert_eq!(virt.total_line_count(width), rows.len());
                let rendered = backend_rows(&virt, width, rows.len() as u16, 0);

                // Width 1 cannot represent double-width glyphs; Ratatui blanks them, so
                // only the row count contract is asserted there.
                if width > 1 {
                    let expected: Vec<String> = rows
                        .iter()
                        .map(|line| line.to_string().trim_end().to_string())
                        .collect();
                    assert_eq!(
                        rendered, expected,
                        "buffer rows diverged from canonical rows at width {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn virtualized_scroll_offsets_match_canonical_row_slices() {
        let items = vec![
            TranscriptItem::user("first message with enough words to wrap at narrow widths"),
            TranscriptItem::assistant(
                "second message with a long paragraph that wraps and wraps and wraps",
            ),
            TranscriptItem::thinking("third block of reasoning that also wraps for several rows"),
            TranscriptItem::notice("fourth notice", NoticeLevel::Info),
            TranscriptItem::assistant("fifth answer"),
        ];
        let virt = TranscriptVirtualizer::from_transcript_items(&items);
        let width = 24u16;
        let viewport = 6u16;
        let all = canonical_rows(&virt, width);
        let total = virt.total_line_count(width);
        assert_eq!(all.len(), total);

        for offset in [
            0,
            1,
            total / 2,
            total.saturating_sub(viewport as usize),
            total,
        ] {
            let rendered = backend_rows(&virt, width, viewport, offset);
            let expected: Vec<String> = all
                .iter()
                .skip(offset)
                .take(viewport as usize)
                .cloned()
                .chain(std::iter::repeat(String::new()))
                .take(viewport as usize)
                .collect();
            assert_eq!(rendered, expected, "scroll offset {offset} diverged");
        }
    }

    #[test]
    fn get_line_as_string_matches_canonical_rows_and_hit_testing() {
        let items = vec![
            TranscriptItem::user("alpha beta gamma delta epsilon"),
            TranscriptItem::assistant("assistant response with words that wrap"),
            TranscriptItem::notice("tail notice", NoticeLevel::Success),
        ];
        let mut virt = TranscriptVirtualizer::from_transcript_items(&items);
        let width = 16u16;
        let all = canonical_rows(&virt, width);

        for (index, expected) in all.iter().enumerate() {
            assert_eq!(
                virt.get_line_as_string(index, width).as_deref(),
                Some(expected.as_str()),
                "line {index}"
            );
            let local = {
                let (_, local) = virt.find_cell_and_local_line(index, width).unwrap();
                local
            };
            let mut start = 0usize;
            let mut cell_index = 0usize;
            for (idx, cell) in virt.cells().iter().enumerate() {
                let count = TranscriptVirtualizer::measure_cell_rendered_height(cell, width);
                if start + count > index {
                    cell_index = idx;
                    break;
                }
                start += count;
            }
            assert!(cell_index < virt.cells().len());
            assert!(
                local
                    < TranscriptVirtualizer::measure_cell_rendered_height(
                        &virt.cells()[cell_index],
                        width
                    )
            );
        }
        assert_eq!(virt.get_line_as_string(all.len(), width), None);
    }

    #[test]
    fn collapse_toggles_invalidate_only_affected_metrics() {
        let items = vec![
            TranscriptItem::ToolCall(ToolCallEntry {
                id: "call-1".into(),
                name: "exec_command".into(),
                arguments: "{\"command\":\"printf line\"}".into(),
                output: Some(
                    (0..10)
                        .map(|i| format!("output line {i}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                success: Some(true),
                in_progress: false,
                collapsed: false,
                duration_ms: Some(8),
            }),
            TranscriptItem::assistant("stable answer"),
        ];
        let mut virt = TranscriptVirtualizer::from_transcript_items(&items);
        let width = 40u16;
        let expanded_total = virt.total_line_count(width);
        let assistant_height =
            TranscriptVirtualizer::measure_cell_rendered_height(&virt.cells()[1], width);

        assert!(virt.toggle_cell(0));
        let collapsed_total = virt.total_line_count(width);
        assert!(collapsed_total < expanded_total);
        assert_eq!(
            assistant_height,
            TranscriptVirtualizer::measure_cell_rendered_height(&virt.cells()[1], width)
        );
        assert!(!virt.toggle_cell(99));
    }

    #[test]
    fn wrapping_preserves_styles_and_never_splits_graphemes() {
        let red = Style::default().fg(Color::Red);
        let blue = Style::default().fg(Color::Blue);
        let line = Line::from(vec![
            Span::styled("ABCDEFGH", red),
            Span::styled("IJKLMNOP", blue),
        ]);
        let rows = wrap_line_to_visual_rows(line, 4);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].to_string(), "ABCD");
        assert_eq!(rows[1].to_string(), "EFGH");
        assert_eq!(rows[2].to_string(), "IJKL");
        assert_eq!(rows[3].to_string(), "MNOP");
        assert!(rows[0].spans.iter().all(|s| s.style == red));
        assert!(rows[2].spans.iter().all(|s| s.style == blue));

        let combining = Line::from("e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}");
        let rows = wrap_line_to_visual_rows(combining, 2);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].to_string(), "e\u{301}e\u{301}");
        assert_eq!(rows[2].to_string(), "e\u{301}");

        let cjk = Line::from("你好世界");
        let rows = wrap_line_to_visual_rows(cjk, 4);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].to_string(), "你好");

        let family = "👨‍👩‍👧‍👦";
        let rows = wrap_line_to_visual_rows(Line::from(family), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].to_string(), family);
        // A glyph wider than the terminal is emitted alone rather than split.
        let rows = wrap_line_to_visual_rows(Line::from(family), 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].to_string(), family);

        let words = Line::from("alpha beta gamma");
        let rows = wrap_line_to_visual_rows(words, 10);
        assert_eq!(
            rows.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
            vec!["alpha beta".to_string(), "gamma".to_string()]
        );
    }

    #[test]
    fn test_extract_code_blocks() {
        let md = r#"
Here is some Rust code:
```rust
fn main() {
    println!("Hello, M31A!");
}
```
And some python:
```python
def add(a, b):
    return a + b
```
"#;
        let blocks = extract_code_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language.as_deref(), Some("rust"));
        assert!(blocks[0].code.contains("fn main()"));
        assert_eq!(blocks[1].language.as_deref(), Some("python"));
        assert!(blocks[1].code.contains("def add(a, b):"));
    }

    #[test]
    fn test_find_last_code_block() {
        let items = vec![
            TranscriptItem::user("Can you write a rust function?"),
            TranscriptItem::assistant("Sure:\n```rust\nfn test() {}\n```"),
            TranscriptItem::notice("Done", NoticeLevel::Success),
        ];

        let virtualizer = TranscriptVirtualizer::from_transcript_items(&items);
        let last_code = virtualizer.find_last_code_block();
        assert!(last_code.is_some());
        let block = last_code.unwrap();
        assert_eq!(block.language.as_deref(), Some("rust"));
        assert_eq!(block.code, "fn test() {}");
    }

    #[test]
    fn test_tool_call_collapse_and_expand() {
        let entry = ToolCallEntry {
            id: "call-1".into(),
            name: "read_file".into(),
            arguments: "{\"path\": \"Cargo.toml\"}".into(),
            output: Some("line 1\nline 2\nline 3\nline 4\nline 5".into()),
            success: Some(true),
            in_progress: false,
            collapsed: true,
            duration_ms: Some(15),
        };

        let mut cell = TranscriptCell::tool_call(&entry);
        assert!(cell.is_collapsed());

        let lines_collapsed = TranscriptVirtualizer::render_cell_visual_rows_static(&cell, 80);
        // In collapsed mode, only 2 lines (summary + blank separator)
        assert_eq!(lines_collapsed.len(), 2);
        assert!(
            lines_collapsed[0]
                .to_string()
                .contains("▶ tool_call read_file")
        );

        // Toggle to expanded
        cell.toggle_collapsed();
        assert!(!cell.is_collapsed());

        let lines_expanded = TranscriptVirtualizer::render_cell_visual_rows_static(&cell, 80);
        assert!(lines_expanded.len() > 3);
        assert!(
            lines_expanded[0]
                .to_string()
                .contains("▼ tool_call read_file")
        );
    }

    #[test]
    fn test_virtualized_visible_window_calculation() {
        let items = vec![
            TranscriptItem::user("First message"),
            TranscriptItem::assistant("Response 1"),
            TranscriptItem::user("Second message"),
            TranscriptItem::assistant("Response 2"),
            TranscriptItem::user("Third message"),
            TranscriptItem::assistant("Response 3"),
        ];

        let virt = TranscriptVirtualizer::from_transcript_items(&items);
        assert_eq!(virt.cells().len(), 6);

        let total_lines = virt.total_line_count(80);
        assert!(total_lines >= 12);

        // Window at offset 0 with viewport height 5
        let (start, end) = virt.calculate_visible_window(0, 5, 80);
        assert_eq!(start, 0);
        assert!(end > 0 && end <= 3);

        // Window towards end
        let (start_tail, end_tail) =
            virt.calculate_visible_window(total_lines.saturating_sub(4), 5, 80);
        assert!(start_tail > 0);
        assert_eq!(end_tail, 6);
    }

    #[test]
    fn test_collapse_output_tail() {
        let text = (1..=200)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (omitted, tail) = collapse_output_tail(&text, 100);
        assert_eq!(omitted, Some(100));
        assert_eq!(tail.len(), 100);
        assert_eq!(tail.last(), Some(&"Line 200"));

        let lines_slice: Vec<&str> = text.lines().collect();
        let (omitted_ht, head, tail_ht) = collapse_output_head_tail(&lines_slice, 20, 80);
        assert_eq!(omitted_ht, Some(100));
        assert_eq!(head.len(), 20);
        assert_eq!(tail_ht.len(), 80);
        assert_eq!(head[0], "Line 1");
        assert_eq!(tail_ht[79], "Line 200");
    }

    #[test]
    fn test_extract_reasoning_title() {
        let reasoning =
            "### Step 1: Check dependencies\nWe need to verify Cargo.toml\nSecond sentence.";
        let title = extract_reasoning_title(reasoning);
        assert_eq!(title.as_deref(), Some("Step 1: Check dependencies"));

        let single_line = "Analyzing the codebase architecture thoroughly.";
        let title2 = extract_reasoning_title(single_line);
        assert_eq!(
            title2.as_deref(),
            Some("Analyzing the codebase architecture thoroughly")
        );
    }

    #[test]
    fn test_exploration_grouping() {
        let items = vec![
            TranscriptItem::user("Find all tests"),
            TranscriptItem::tool_call("c1", "file_search", "{\"query\": \"test\"}"),
            TranscriptItem::tool_call("c2", "read_file", "{\"path\": \"src/main.rs\"}"),
            TranscriptItem::tool_call("c3", "list_dir", "{\"path\": \"crates\"}"),
            TranscriptItem::assistant("Here are the results"),
        ];

        let virt = TranscriptVirtualizer::from_transcript_items(&items);
        // User (1) + ExplorationGroup (1 with 3 tools) + Assistant (1) = 3 cells
        assert_eq!(virt.cells().len(), 3);
        match &virt.cells()[1] {
            TranscriptCell::ExplorationGroup {
                tools, collapsed, ..
            } => {
                assert_eq!(tools.len(), 3);
                assert!(*collapsed);
            }
            other => panic!("Expected ExplorationGroup, got {:?}", other),
        }

        // Expanded view shows all 3 tools
        let mut group = virt.cells()[1].clone();
        group.toggle_collapsed();
        let lines = TranscriptVirtualizer::render_cell_visual_rows_static(&group, 100);
        assert!(lines.len() >= 4);
        let rendered = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("file_search"));
        assert!(rendered.contains("read_file"));
        assert!(rendered.contains("list_dir"));
    }

    #[test]
    fn test_cached_line_metrics() {
        let items = vec![
            TranscriptItem::user("Hello"),
            TranscriptItem::assistant("World\nWith multiple lines\nof text."),
        ];
        let virt = TranscriptVirtualizer::from_transcript_items(&items);
        assert!(virt.cached_total_lines.get().is_none());

        let lines1 = virt.total_line_count(80);
        assert!(lines1 > 0);
        assert_eq!(virt.cached_total_lines.get(), Some(lines1));

        // Subsequent call reuses cache
        let lines2 = virt.total_line_count(80);
        assert_eq!(lines1, lines2);

        // Invalidation clears cache
        virt.invalidate_cache();
        assert!(virt.cached_total_lines.get().is_none());
    }

    #[test]
    fn test_adversarial_code_block_extraction() {
        // 1. Tildes code block
        let text_tilde = "~~~python\nprint('hello')\n~~~";
        let blocks = extract_code_blocks(text_tilde);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
        assert_eq!(blocks[0].code, "print('hello')");

        // 2. 4 backticks containing 3 backticks inside
        let text_nested = "````markdown\nHere is a block:\n```rust\nfn main() {}\n```\n````";
        let blocks = extract_code_blocks(text_nested);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("markdown"));
        assert!(blocks[0].code.contains("```rust"));

        // 3. Unclosed code block during streaming
        let text_unclosed = "```typescript\nconst x = 42;";
        let blocks = extract_code_blocks(text_unclosed);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("typescript"));
        assert_eq!(blocks[0].code, "const x = 42;");
    }

    #[test]
    fn test_exploration_group_expansion_preservation() {
        let items = vec![
            TranscriptItem::tool_call("t1", "read_file", "{}"),
            TranscriptItem::tool_call("t2", "list_dir", "{}"),
        ];

        let mut virt = TranscriptVirtualizer::from_transcript_items(&items);
        assert_eq!(virt.cells().len(), 1);
        assert!(virt.cells()[0].is_collapsed());

        // Toggle to expand
        let group_id = match &virt.cells()[0] {
            TranscriptCell::ExplorationGroup { id, .. } => id.clone(),
            _ => panic!("Expected ExplorationGroup"),
        };
        virt.toggle_exploration_group(&group_id);
        assert!(!virt.cells()[0].is_collapsed());
        assert!(virt.expanded_ids.contains(&group_id));

        // Re-creating virtualizer with expanded_ids preserves the expanded state!
        let virt2 = TranscriptVirtualizer::from_transcript_items_preserving_state(
            &items,
            &virt.expanded_ids,
        );
        assert_eq!(virt2.cells.len(), 1);
        assert!(!virt2.cells[0].is_collapsed());
    }
}
