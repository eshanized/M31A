//! Bounded Code Block and Tool Output Viewer (TUI-01, TDS-02).
//!
//! Enforces Law 5 and Section 8/28: Large outputs (up to 500MB) must never
//! flood the framebuffer or block the render path. Provides clean truncation,
//! artifact locator citations, and structured terminal styling.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::sanitizer::sanitize_terminal_text;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Renders a tool output block, enforcing line bounds and artifact citations for large output.
pub fn render_bounded_output<'a>(
    content: &str,
    max_lines: usize,
    width: u16,
    artifact_id: Option<&str>,
    tokens: &ThemeTokens,
) -> Vec<Line<'a>> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let total_bytes = content.len();
    let line_limit = max_lines.max(3);

    let mut lines = Vec::new();
    let mut sampled_bytes = 0;
    let mut sampled_lines = 0;

    // Fast path (Law 5): only parse and sanitize the lines actually displayed
    for raw_line in content.lines().take(line_limit) {
        sampled_bytes += raw_line.len() + 1;
        sampled_lines += 1;
        let sanitized = sanitize_terminal_text(raw_line);
        let max_chars = width.saturating_sub(4) as usize;
        let truncated = if sanitized.chars().count() > width as usize && max_chars > 3 {
            let kept: String = sanitized
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect();
            format!("{kept}...")
        } else if sanitized.chars().count() > width as usize {
            sanitized.chars().take(width as usize).collect()
        } else {
            sanitized
        };

        // Style known status patterns
        let style = if is_mono {
            Style::default()
        } else if truncated.contains("error")
            || truncated.contains("FAILED")
            || truncated.starts_with("Error:")
        {
            tokens.status_failed
        } else if truncated.contains("ok")
            || truncated.contains("passed")
            || truncated.contains("SUCCESS")
        {
            tokens.status_ok
        } else if truncated.contains("warning") || truncated.contains("WARN") {
            tokens.status_warning
        } else {
            tokens.text_secondary
        };

        lines.push(Line::from(vec![
            Span::styled("  ", tokens.text_muted),
            Span::styled(truncated, style),
        ]));
    }

    // Fast line count: exact for small outputs, sampled estimation for massive outputs (Law 5, TRD §28)
    let total_lines = if total_bytes <= 16 * 1024 {
        content.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    } else if sampled_lines > 0 && sampled_bytes > 0 {
        let avg_line_len = (sampled_bytes / sampled_lines).max(1);
        sampled_lines + (total_bytes.saturating_sub(sampled_bytes) / avg_line_len)
    } else {
        total_bytes / 80
    };

    // If truncated or large output, render externalized citation banner
    if total_lines > line_limit || total_bytes > 4096 {
        let remaining_lines = total_lines.saturating_sub(line_limit);
        let size_str = if total_bytes >= 1_048_576 {
            format!("{:.1} MB", total_bytes as f64 / 1_048_576.0)
        } else if total_bytes >= 1024 {
            format!("{:.1} KB", total_bytes as f64 / 1024.0)
        } else {
            format!("{total_bytes} bytes")
        };

        let citation = if let Some(art) = artifact_id {
            let art_str = if art.starts_with("artifact://") {
                art.to_string()
            } else {
                format!("artifact://{art}")
            };
            format!("... +{remaining_lines} lines ({size_str}) → {art_str} [o to inspect]")
        } else {
            format!("... +{remaining_lines} lines ({size_str}) [truncated to preserve viewport]")
        };

        let banner_style = tokens.text_muted;

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(citation, banner_style),
        ]));
    }

    lines
}
