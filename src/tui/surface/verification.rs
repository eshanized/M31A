//! Verification Gates & Proof Evidence Surface (Section 25, TUI-01).
//!
//! Visualizes independent verification gates, checks, execution evidence,
//! failure classifications, and proof artifacts backed by canonical runtime state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::{TuiVerificationCheck, TuiViewModel};
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Verification Gates surface.
pub fn render_verification_surface(
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

    let title = format!(
        " Verification Gates & Evidence ({} checks | overall: {}) ",
        model.verification_checks.len(),
        model.verification_summary.overall_status
    );

    // Explicit empty state (Section 4 & 6: never infer no record == passed)
    if model.verification_checks.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  No verification checks recorded (status: not run / pending).",
                tokens.status_warning.add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(
                "  Verification completion is strictly evidence-backed (VER-01..05, Law 6).",
                tokens.text_primary,
            ),
            Line::styled(
                "  Rule: Absence of verification evidence is NEVER inferred as success.",
                tokens.status_failed.add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "  Checks run deterministically through the 7-tier VerificationHierarchyEngine.",
                tokens.text_secondary,
            ),
            Line::styled(
                "  Deterministic (1) → Compiler (2) → Tests (3) → StaticAnalysis (4) → DiffInvariants (5) → Review (6) → Diagnosis (7)",
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

    let checks = &model.verification_checks;

    if area.width >= 70 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_checks_list(f, chunks[0], checks, tokens, selected_idx, border_style);
        render_check_evidence(
            f,
            chunks[1],
            checks,
            model,
            tokens,
            selected_idx,
            border_style,
        );
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_checks_list(f, chunks[0], checks, tokens, selected_idx, border_style);
        render_check_evidence(
            f,
            chunks[1],
            checks,
            model,
            tokens,
            selected_idx,
            border_style,
        );
    }
}

fn render_checks_list(
    f: &mut Frame,
    area: Rect,
    checks: &[TuiVerificationCheck],
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let mut lines = Vec::new();
    let max_name_w = 22.min(area.width.saturating_sub(22) as usize);

    for (i, check) in checks.iter().enumerate() {
        let is_selected = i == selected_idx.min(checks.len().saturating_sub(1));

        // Match arms are the canonical CheckStatus::as_str()
        // values (passed/failed/blocked/not_run/skipped_with_reason).
        // Unrecognized persisted strings render as UNRECOGNIZED and are
        // never conflated with a legitimate NOT RUN check.
        let (icon, sym_style) = match check.status.as_str() {
            "passed" => ("✓ ", tokens.status_ok),
            "failed" => ("✗ ", tokens.status_failed),
            "blocked" => ("⊘ ", tokens.status_failed),
            "not_run" => ("○ ", tokens.status_waiting),
            "skipped_with_reason" => ("⊝ ", tokens.text_muted),
            _ => ("? ", tokens.status_warning),
        };

        let display_name = if check.command_or_tool.is_empty() {
            &check.check_id
        } else {
            &check.command_or_tool
        };

        let truncated = if display_name.len() > max_name_w {
            format!("{:.w$}...", display_name, w = max_name_w.saturating_sub(3))
        } else {
            display_name.to_string()
        };

        let row_style = if is_selected {
            Style::default()
                .bg(tokens.bg_surface.bg.unwrap_or(Color::DarkGray))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let prefix = if is_selected { "▸ " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled(prefix, tokens.accent_primary),
            Span::styled(icon, sym_style),
            Span::styled(format!("{:<w$} ", truncated, w = max_name_w), row_style),
            Span::styled(format!("T{} ", check.tier_num), tokens.text_secondary),
            Span::styled(format!("[{}]", check.status), sym_style),
        ]));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(format!(" Verification ({}) ", checks.len()))
            .borders(Borders::NONE)
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn render_check_evidence(
    f: &mut Frame,
    area: Rect,
    checks: &[TuiVerificationCheck],
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    selected_idx: usize,
    border_style: Style,
) {
    let idx = selected_idx.min(checks.len().saturating_sub(1));
    let check = &checks[idx];

    let (status_icon, status_style) = match check.status.as_str() {
        "passed" => ("PASSED ✓", tokens.status_ok),
        "failed" => ("FAILED ✗", tokens.status_failed),
        "blocked" => ("BLOCKED ⊘", tokens.status_failed),
        "not_run" => ("NOT RUN ○", tokens.status_waiting),
        "skipped_with_reason" => ("SKIPPED ⊝", tokens.text_muted),
        _ => ("UNRECOGNIZED ?", tokens.status_warning),
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Check Status     : ", tokens.text_muted),
        Span::styled(status_icon, status_style.add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Check ID         : ", tokens.text_muted),
        Span::styled(&check.check_id, tokens.accent_primary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Hierarchy Tier   : ", tokens.text_muted),
        Span::styled(
            format!("Tier {} ({})", check.tier_num, check.tier_name),
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Command / Tool   : ", tokens.text_muted),
        Span::styled(&check.command_or_tool, tokens.accent_secondary),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Target / Task ID : ", tokens.text_muted),
        Span::styled(
            if check.task_id.is_empty() {
                "[None]".to_string()
            } else {
                check.task_id.clone()
            },
            tokens.text_secondary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Mission ID       : ", tokens.text_muted),
        Span::styled(
            if check.mission_id.is_empty() {
                "[None]".to_string()
            } else {
                check.mission_id.clone()
            },
            tokens.text_muted,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Inputs Normalized: ", tokens.text_muted),
        Span::styled(&check.inputs_normalized, tokens.text_muted),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Evidence Artifact: ", tokens.text_muted),
        Span::styled(
            check.evidence_artifact_id.as_deref().unwrap_or("[None]"),
            tokens.text_primary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Failure Class    : ", tokens.text_muted),
        Span::styled(
            check.failure_class.as_deref().unwrap_or("[None]"),
            if check.failure_class.is_some() {
                tokens.status_failed
            } else {
                tokens.text_muted
            },
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Snapshot Hash    : ", tokens.text_muted),
        Span::styled(
            if check.snapshot_hash.is_empty() {
                "[Not bound]".to_string()
            } else {
                check.snapshot_hash.clone()
            },
            tokens.accent_secondary,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Created At (UTC) : ", tokens.text_muted),
        Span::styled(check.created_at.to_rfc3339(), tokens.text_muted),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "SUMMARY & EVIDENCE DETAILS:",
        tokens.accent_primary.add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::styled(
        format!("  {}", check.summary),
        tokens.text_primary,
    ));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!(
            "GATE METRICS: passed={}, failed={}, blocked={}, pending={}, total={}",
            model.verification_summary.passed_count,
            model.verification_summary.failed_count,
            model.verification_summary.blocked_count,
            model.verification_summary.not_run_count,
            model.verification_summary.total_checks
        ),
        tokens.text_secondary,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Evidence [{}] ", check.check_id))
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
