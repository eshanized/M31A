//! Post-Mortem Event Replay Surface (Section 27, TUI-01, T-11-20).
//!
//! Visualizes historical event timelines with step-by-step playback:
//! - Clear, unmistakable [REPLAY / POST-MORTEM - READ-ONLY] indicator
//! - Play/pause, step forward/backward, speed multiplier controls
//! - State reconstruction inspection at each historical frame

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::replay::ReplayController;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Post-Mortem Replay surface.
pub fn render_replay_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    replay: &ReplayController,
    tokens: &ThemeTokens,
    is_focused: bool,
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

    let banner_style = tokens.warning;

    let mut lines = Vec::new();
    lines.push(Line::styled(" Replay · read-only", banner_style));
    lines.push(Line::raw(""));

    let total_frames = replay.history.len();
    let current_cursor = if total_frames > 0 {
        replay.cursor + 1
    } else {
        0
    };

    lines.push(Line::from(vec![
        Span::styled("Timeline Cursor : ", tokens.text_muted),
        Span::styled(
            format!("{current_cursor} / {total_frames} events"),
            tokens.accent_primary.add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Status: "),
        Span::styled(
            if replay.is_playing {
                "PLAYING ▶"
            } else {
                "PAUSED ⏸"
            },
            tokens.status_running,
        ),
        Span::raw(" | Speed: "),
        Span::styled(replay.speed.as_str(), tokens.text_primary),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Historical Playback Controls:",
        tokens.text_muted,
    ));
    lines.push(Line::styled(
        "  • [Space] : Play / Pause timeline playback",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • [ ]     : Step backward one event",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • [ ] ]   : Step forward one event",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • [ > ]   : Cycle playback speed (1x, 2x, 5x, 10x)",
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        "  • [Esc]   : Exit replay mode and return to live cockpit",
        tokens.text_secondary,
    ));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Reconstructed State At Cursor:",
        tokens.text_muted,
    ));
    lines.push(Line::styled(
        format!("  Mission Status : {}", model.mission_status.to_uppercase()),
        tokens.text_primary,
    ));
    lines.push(Line::styled(
        format!("  Tasks in DAG   : {}", model.tasks.len()),
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        format!("  Active Agents  : {}", model.agents.len()),
        tokens.text_secondary,
    ));
    lines.push(Line::styled(
        format!("  Recorded Logs  : {}", model.logs.len()),
        tokens.text_secondary,
    ));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Replay ")
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
