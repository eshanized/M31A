//! Artifact Explorer & Output Management Surface (Section 28, TUI-01).
//!
//! Visualizes generated artifacts, reports, test logs, and patches without
//! freezing the UI or dumping massive outputs into the terminal buffer.
//! Authoritative data derives purely from canonical runtime state (ArtifactService / TuiViewModel).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::{TuiArtifactSnapshot, TuiViewModel};
use crate::tui::theme::{ThemeMode, ThemeTokens};

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Render the Artifact Browser surface backed by canonical runtime state.
pub fn render_artifacts_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_focused: bool,
    selected_idx: usize,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    let title = format!(" Artifacts ({} records) ", model.artifacts.len());

    // Explicit empty state (Section 4 & 5: never manufacture data)
    if model.artifacts.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  No artifacts recorded in runtime store.",
                tokens.text_muted.add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(
                "  Artifacts are produced during autonomous execution and tracked in SQLite.",
                tokens.text_secondary,
            ),
            Line::styled(
                "  Canonical store: FsArtifactStore (.m31a/artifacts/) bound to SHA-256 ledger.",
                tokens.text_muted,
            ),
            Line::styled(
                "  Empty state reflects absence of durable records, not an error.",
                tokens.text_muted,
            ),
        ])
        .block(
            Block::default()
                .title(title)
                .borders(Borders::NONE)
                .border_style(border_style),
        );
        f.render_widget(p, area);
        return;
    }

    let artifacts = &model.artifacts;

    if area.width >= 70 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_artifacts_list(f, chunks[0], artifacts, tokens, selected_idx, border_style);
        render_artifact_detail(f, chunks[1], artifacts, tokens, selected_idx, border_style);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_artifacts_list(f, chunks[0], artifacts, tokens, selected_idx, border_style);
        render_artifact_detail(f, chunks[1], artifacts, tokens, selected_idx, border_style);
    }
}

fn render_artifacts_list(
    f: &mut Frame,
    area: Rect,
    artifacts: &[TuiArtifactSnapshot],
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let mut lines = Vec::new();
    let max_name_w = 22.min(area.width.saturating_sub(20) as usize);

    for (i, art) in artifacts.iter().enumerate() {
        let is_selected = i == selected_idx.min(artifacts.len().saturating_sub(1));
        let truncated = if art.name.len() > max_name_w {
            format!("{:.w$}...", art.name, w = max_name_w.saturating_sub(3))
        } else {
            art.name.clone()
        };

        let row_style = if is_selected {
            Style::default()
                .bg(tokens.bg_surface.bg.unwrap_or(Color::DarkGray))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let prefix = if is_selected { "▸ " } else { "  " };
        let size_str = format_bytes(art.size_bytes);

        lines.push(Line::from(vec![
            Span::styled(prefix, tokens.accent_primary),
            Span::styled(format!("{:<w$} ", truncated, w = max_name_w), row_style),
            Span::styled(format!("v{} ", art.version), tokens.text_secondary),
            Span::styled(format!("{:>8}", size_str), tokens.text_muted),
        ]));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(format!(" Artifacts ({}) ", artifacts.len()))
            .borders(Borders::NONE)
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn render_artifact_detail(
    f: &mut Frame,
    area: Rect,
    artifacts: &[TuiArtifactSnapshot],
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let idx = selected_idx.min(artifacts.len().saturating_sub(1));
    let art = &artifacts[idx];

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("Artifact Identity : {}", art.id),
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::styled(
        format!("Name              : {}", art.name),
        tokens.text_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::styled(
        format!("Logical Path      : {}", art.logical_path),
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        format!("Content SHA-256   : {}", art.content_hash),
        tokens.accent_secondary,
    ));
    lines.push(Line::styled(
        format!(
            "Size / Version    : {} (v{})",
            format_bytes(art.size_bytes),
            art.version
        ),
        tokens.text_primary,
    ));
    lines.push(Line::styled(
        format!("Status / Storage  : {} ({})", art.status, art.storage_state),
        tokens.status_ok,
    ));
    lines.push(Line::styled(
        format!("Created Timestamp : {}", art.created_at.to_rfc3339()),
        tokens.text_muted,
    ));
    lines.push(Line::raw(""));

    lines.push(Line::styled(
        "PROVENANCE & CAUSALITY (D-13):",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::styled(
        format!(
            "  Mission ID      : {}",
            art.mission_id.as_deref().unwrap_or("[None]")
        ),
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        format!(
            "  Task ID         : {}",
            art.task_id.as_deref().unwrap_or("[None]")
        ),
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        format!(
            "  Producer Role   : {}",
            art.producer_role.as_deref().unwrap_or("[None]")
        ),
        tokens.text_secondary,
    ));

    let parents_str = if art.parent_artifact_ids.is_empty() {
        "[None]".to_string()
    } else {
        art.parent_artifact_ids.join(", ")
    };
    lines.push(Line::styled(
        format!("  Parent Artifacts: {}", parents_str),
        tokens.text_muted,
    ));

    let checks_str = if art.verification_check_ids.is_empty() {
        "[None]".to_string()
    } else {
        art.verification_check_ids.join(", ")
    };
    lines.push(Line::styled(
        format!("  Verified Checks : {}", checks_str),
        tokens.text_muted,
    ));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "VIEWPORT PROTECTION (Law 15):",
        tokens.text_muted,
    ));
    lines.push(Line::styled(
        "  • In-memory projection only; zero synchronous disk I/O on render.",
        tokens.text_muted,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Details [{}] ", art.name))
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
