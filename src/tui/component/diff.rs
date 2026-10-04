//! Production-Grade Unified Diff Presentation Component (TUI-01, TDS-03, Section 24).
//!
//! Parses and renders unified diffs with syntax coloring, line numbers,
//! hunk headers, file attribution, and addition/deletion telemetry.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::sanitizer::sanitize_diff;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// A single parsed diff line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineType {
    Header,
    HunkHeader { old_start: usize, new_start: usize },
    Addition { line_no: usize },
    Deletion { line_no: usize },
    Context { old_line: usize, new_line: usize },
    Comment,
}

/// A parsed line with its type and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDiffLine {
    pub line_type: DiffLineType,
    pub content: String,
}

/// Statistics for a diff file or hunk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffStats {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}

/// High-level parser for unified diff text.
pub struct DiffParser;

impl DiffParser {
    pub fn parse(diff_text: &str) -> (Vec<ParsedDiffLine>, DiffStats) {
        let sanitized = sanitize_diff(diff_text);
        let mut lines = Vec::new();
        let mut stats = DiffStats::default();

        let mut current_old_line = 0usize;
        let mut current_new_line = 0usize;

        for raw_line in sanitized.lines() {
            if raw_line.starts_with("diff --git") {
                stats.files_changed += 1;
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::Header,
                    content: raw_line.to_string(),
                });
            } else if raw_line.starts_with("--- ") || raw_line.starts_with("+++ ") {
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::Header,
                    content: raw_line.to_string(),
                });
            } else if raw_line.starts_with("@@") {
                // Parse @@ -old,len +new,len @@
                let (old_st, new_st) = parse_hunk_indices(raw_line);
                current_old_line = old_st;
                current_new_line = new_st;
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::HunkHeader {
                        old_start: old_st,
                        new_start: new_st,
                    },
                    content: raw_line.to_string(),
                });
            } else if let Some(stripped) = raw_line.strip_prefix('+') {
                stats.additions += 1;
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::Addition {
                        line_no: current_new_line,
                    },
                    content: stripped.to_string(),
                });
                current_new_line += 1;
            } else if let Some(stripped) = raw_line.strip_prefix('-') {
                stats.deletions += 1;
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::Deletion {
                        line_no: current_old_line,
                    },
                    content: stripped.to_string(),
                });
                current_old_line += 1;
            } else {
                let stripped = raw_line.strip_prefix(' ').unwrap_or(raw_line);
                lines.push(ParsedDiffLine {
                    line_type: DiffLineType::Context {
                        old_line: current_old_line,
                        new_line: current_new_line,
                    },
                    content: stripped.to_string(),
                });
                current_old_line += 1;
                current_new_line += 1;
            }
        }

        (lines, stats)
    }
}

fn parse_hunk_indices(header: &str) -> (usize, usize) {
    let mut old_start = 1;
    let mut new_start = 1;

    let parts: Vec<&str> = header.split("@@").collect();
    if parts.len() >= 2 {
        let spec = parts[1].trim();
        for chunk in spec.split_whitespace() {
            if let Some(s) = chunk.strip_prefix('-')
                && let Some(num_str) = s.split(',').next()
            {
                old_start = num_str.parse().unwrap_or(1);
            } else if let Some(s) = chunk.strip_prefix('+')
                && let Some(num_str) = s.split(',').next()
            {
                new_start = num_str.parse().unwrap_or(1);
            }
        }
    }

    (old_start, new_start)
}

/// Render parsed diff lines into formatted Ratatui lines with line numbers and syntax styling.
pub fn render_diff_lines<'a>(
    parsed_lines: &[ParsedDiffLine],
    width: u16,
    tokens: &ThemeTokens,
) -> Vec<Line<'a>> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let mut result = Vec::with_capacity(parsed_lines.len());

    for item in parsed_lines {
        match &item.line_type {
            DiffLineType::Header => {
                let style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    tokens.accent_primary.add_modifier(Modifier::BOLD)
                };
                result.push(Line::from(vec![
                    Span::styled("        ", tokens.text_muted),
                    Span::styled(item.content.clone(), style),
                ]));
            }
            DiffLineType::HunkHeader { .. } => {
                let style = if is_mono {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    tokens.text_muted
                };
                result.push(Line::from(vec![
                    Span::styled("   @@   ", tokens.text_muted),
                    Span::styled(item.content.clone(), style),
                ]));
            }
            DiffLineType::Addition { line_no } => {
                let (prefix_style, text_style) = if is_mono {
                    (
                        Style::default().add_modifier(Modifier::BOLD),
                        Style::default(),
                    )
                } else {
                    (
                        tokens.diff_addition.add_modifier(Modifier::BOLD),
                        tokens.diff_addition,
                    )
                };
                result.push(Line::from(vec![
                    Span::styled(format!("{line_no:>4} + "), prefix_style),
                    Span::styled(
                        truncate_str(&item.content, width.saturating_sub(8)),
                        text_style,
                    ),
                ]));
            }
            DiffLineType::Deletion { line_no } => {
                let (prefix_style, text_style) = if is_mono {
                    (
                        Style::default().add_modifier(Modifier::DIM),
                        Style::default(),
                    )
                } else {
                    (
                        tokens.diff_deletion.add_modifier(Modifier::BOLD),
                        tokens.diff_deletion,
                    )
                };
                result.push(Line::from(vec![
                    Span::styled(format!("{line_no:>4} - "), prefix_style),
                    Span::styled(
                        truncate_str(&item.content, width.saturating_sub(8)),
                        text_style,
                    ),
                ]));
            }
            DiffLineType::Context { new_line, .. } => {
                result.push(Line::from(vec![
                    Span::styled(format!("{new_line:>4}   "), tokens.text_muted),
                    Span::styled(
                        truncate_str(&item.content, width.saturating_sub(8)),
                        tokens.text_secondary,
                    ),
                ]));
            }
            DiffLineType::Comment => {
                result.push(Line::from(vec![
                    Span::styled("        ", tokens.text_muted),
                    Span::styled(
                        truncate_str(&item.content, width.saturating_sub(8)),
                        tokens.text_muted,
                    ),
                ]));
            }
        }
    }

    result
}

fn truncate_str(s: &str, max: u16) -> String {
    let limit = max as usize;
    if s.len() > limit && limit > 3 {
        format!("{}...", &s[..limit.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
