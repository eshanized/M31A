//! Interactive TUI Diff Renderer
//!
//! Parses git unified diffs into structured hunks and renders them with line numbers and theme styling.
//! Parses unified git diffs into structured hunks and renders them with dual line numbers,
//! gutter indicators, and theme-aware color highlighting.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::{ColorRole, Theme, theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    FileHeader,
    HunkHeader { old_start: usize, new_start: usize },
    Addition { new_line: usize },
    Deletion { old_line: usize },
    Context { old_line: usize, new_line: usize },
}

#[derive(Debug, Clone)]
pub struct RenderedDiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
}

/// Aggregate metrics for a parsed unified diff patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStatsSummary {
    pub additions: usize,
    pub deletions: usize,
    pub hunks: usize,
    pub files: usize,
}

pub struct DiffRenderer;

impl DiffRenderer {
    /// Computes summary statistics across parsed diff lines.
    pub fn summarize(parsed: &[RenderedDiffLine]) -> DiffStatsSummary {
        let mut stats = DiffStatsSummary::default();
        for item in parsed {
            match item.kind {
                DiffLineKind::Addition { .. } => stats.additions += 1,
                DiffLineKind::Deletion { .. } => stats.deletions += 1,
                DiffLineKind::HunkHeader { .. } => stats.hunks += 1,
                DiffLineKind::FileHeader if item.content.starts_with("diff --git") => {
                    stats.files += 1;
                }
                _ => {}
            }
        }
        if stats.files == 0 && (stats.additions > 0 || stats.deletions > 0 || stats.hunks > 0) {
            stats.files = 1;
        }
        stats
    }

    pub fn parse_unified_diff(diff: &str) -> Vec<RenderedDiffLine> {
        let mut rendered = Vec::new();
        let mut curr_old = 0;
        let mut curr_new = 0;

        for line in diff.lines() {
            if line.starts_with("diff --git")
                || line.starts_with("index ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("new file mode")
                || line.starts_with("deleted file mode")
                || line.starts_with("rename ")
            {
                rendered.push(RenderedDiffLine {
                    kind: DiffLineKind::FileHeader,
                    content: line.to_string(),
                    old_line_num: None,
                    new_line_num: None,
                });
            } else if line.starts_with("@@") {
                // Parse @@ -old_start,old_len +new_start,new_len @@
                let (old_s, new_s) = Self::parse_hunk_header(line);
                curr_old = old_s;
                curr_new = new_s;
                rendered.push(RenderedDiffLine {
                    kind: DiffLineKind::HunkHeader {
                        old_start: old_s,
                        new_start: new_s,
                    },
                    content: line.to_string(),
                    old_line_num: None,
                    new_line_num: None,
                });
            } else if let Some(stripped) = line.strip_prefix('+') {
                rendered.push(RenderedDiffLine {
                    kind: DiffLineKind::Addition { new_line: curr_new },
                    content: stripped.to_string(),
                    old_line_num: None,
                    new_line_num: Some(curr_new),
                });
                curr_new += 1;
            } else if let Some(stripped) = line.strip_prefix('-') {
                rendered.push(RenderedDiffLine {
                    kind: DiffLineKind::Deletion { old_line: curr_old },
                    content: stripped.to_string(),
                    old_line_num: Some(curr_old),
                    new_line_num: None,
                });
                curr_old += 1;
            } else {
                let text = line.strip_prefix(' ').unwrap_or(line);
                rendered.push(RenderedDiffLine {
                    kind: DiffLineKind::Context {
                        old_line: curr_old,
                        new_line: curr_new,
                    },
                    content: text.to_string(),
                    old_line_num: Some(curr_old),
                    new_line_num: Some(curr_new),
                });
                curr_old += 1;
                curr_new += 1;
            }
        }

        rendered
    }

    fn parse_hunk_header(header: &str) -> (usize, usize) {
        // e.g. "@@ -1,5 +1,6 @@ ..."
        let parts: Vec<&str> = header.split("@@").collect();
        if parts.len() >= 2 {
            let inner = parts[1].trim();
            let mut old_start = 1;
            let mut new_start = 1;
            for token in inner.split_whitespace() {
                if let Some(s) = token.strip_prefix('-') {
                    if let Some(num_str) = s.split(',').next() {
                        old_start = num_str.parse().unwrap_or(1);
                    }
                } else if let Some(s) = token.strip_prefix('+')
                    && let Some(num_str) = s.split(',').next()
                {
                    new_start = num_str.parse().unwrap_or(1);
                }
            }
            (old_start, new_start)
        } else {
            (1, 1)
        }
    }

    pub fn render_to_ratatui_lines(parsed: &[RenderedDiffLine]) -> Vec<Line<'static>> {
        let t = theme();
        parsed
            .iter()
            .map(|item| {
                let old_str = match item.old_line_num {
                    Some(n) => format!("{:>4}", n),
                    None => "    ".to_string(),
                };
                let new_str = match item.new_line_num {
                    Some(n) => format!("{:>4}", n),
                    None => "    ".to_string(),
                };

                match item.kind {
                    DiffLineKind::FileHeader => Line::from(vec![
                        Span::styled("        ", t.style_muted()),
                        Span::raw(" "),
                        Span::styled(item.content.clone(), t.style_info(true)),
                    ]),
                    DiffLineKind::HunkHeader { .. } => Line::from(vec![
                        Span::styled("        ", t.style_muted()),
                        Span::raw(" "),
                        Span::styled(
                            item.content.clone(),
                            t.style_accent(false).add_modifier(Modifier::ITALIC),
                        ),
                    ]),
                    DiffLineKind::Addition { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_muted()),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_success(false)),
                        Span::styled(" + ", t.style_success(true).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            item.content.clone(),
                            Style::default()
                                .fg(t.color(ColorRole::Success))
                                .bg(t.diff_add_bg()),
                        ),
                    ]),
                    DiffLineKind::Deletion { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_error(false)),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_muted()),
                        Span::styled(" - ", t.style_error(true).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            item.content.clone(),
                            Style::default()
                                .fg(t.color(ColorRole::Error))
                                .bg(t.diff_del_bg()),
                        ),
                    ]),
                    DiffLineKind::Context { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_muted()),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_muted()),
                        Span::raw("   "),
                        Span::styled(item.content.clone(), t.style_secondary()),
                    ]),
                }
            })
            .collect()
    }

    /// Render with explicit theme (for use in renderer)
    pub fn render_lines(parsed: &[RenderedDiffLine], t: &Theme) -> Vec<Line<'static>> {
        parsed
            .iter()
            .map(|item| {
                let old_str = match item.old_line_num {
                    Some(n) => format!("{:>4}", n),
                    None => "    ".to_string(),
                };
                let new_str = match item.new_line_num {
                    Some(n) => format!("{:>4}", n),
                    None => "    ".to_string(),
                };

                match item.kind {
                    DiffLineKind::FileHeader => Line::from(vec![
                        Span::styled("        ", t.style_muted()),
                        Span::raw(" "),
                        Span::styled(item.content.clone(), t.style_info(true)),
                    ]),
                    DiffLineKind::HunkHeader { .. } => Line::from(vec![
                        Span::styled("        ", t.style_muted()),
                        Span::raw(" "),
                        Span::styled(
                            item.content.clone(),
                            t.style_accent(false).add_modifier(Modifier::ITALIC),
                        ),
                    ]),
                    DiffLineKind::Addition { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_muted()),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_success(false)),
                        Span::styled(" + ", t.style_success(true).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            item.content.clone(),
                            Style::default()
                                .fg(t.color(ColorRole::Success))
                                .bg(t.diff_add_bg()),
                        ),
                    ]),
                    DiffLineKind::Deletion { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_error(false)),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_muted()),
                        Span::styled(" - ", t.style_error(true).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            item.content.clone(),
                            Style::default()
                                .fg(t.color(ColorRole::Error))
                                .bg(t.diff_del_bg()),
                        ),
                    ]),
                    DiffLineKind::Context { .. } => Line::from(vec![
                        Span::styled(old_str, t.style_muted()),
                        Span::raw(" "),
                        Span::styled(new_str, t.style_muted()),
                        Span::raw("   "),
                        Span::styled(item.content.clone(), t.style_secondary()),
                    ]),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unified_diff() {
        let diff = r#"diff --git a/foo.rs b/foo.rs
index 123456..789abc 100644
--- a/foo.rs
+++ b/foo.rs
@@ -10,3 +10,4 @@ fn test() {
 println!("old");
-println!("deleted");
+println!("added 1");
+println!("added 2");
 }"#;

        let parsed = DiffRenderer::parse_unified_diff(diff);
        assert!(parsed.len() >= 8);

        let lines = DiffRenderer::render_to_ratatui_lines(&parsed);
        assert_eq!(lines.len(), parsed.len());

        let adds = parsed
            .iter()
            .filter(|l| matches!(l.kind, DiffLineKind::Addition { .. }))
            .count();
        let dels = parsed
            .iter()
            .filter(|l| matches!(l.kind, DiffLineKind::Deletion { .. }))
            .count();
        assert_eq!(adds, 2);
        assert_eq!(dels, 1);

        let stats = DiffRenderer::summarize(&parsed);
        assert_eq!(stats.additions, 2);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.hunks, 1);
        assert_eq!(stats.files, 1);
    }
}
