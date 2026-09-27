//! Centralized Theme & Design Token System (TDS-03, D-03).
//!
//! Provides the 4 canonical themes (DarkSlateCyan, HighContrast, CleanLight, MonochromeANSI),
//! automated NO_COLOR compliance, and semantic styling tokens.

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

/// Canonical theme options for M31A TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    DarkSlateCyan,
    HighContrast,
    CleanLight,
    MonochromeANSI,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DarkSlateCyan => "Dark Slate Cyan",
            Self::HighContrast => "High Contrast",
            Self::CleanLight => "Clean Light",
            Self::MonochromeANSI => "Monochrome ANSI",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::DarkSlateCyan => Self::HighContrast,
            Self::HighContrast => Self::CleanLight,
            Self::CleanLight => Self::MonochromeANSI,
            Self::MonochromeANSI => Self::DarkSlateCyan,
        }
    }

    pub fn from_str_relaxed(s: &str) -> Self {
        match s.to_lowercase().replace(['-', '_'], "").as_str() {
            "highcontrast" => Self::HighContrast,
            "cleanlight" | "light" => Self::CleanLight,
            "monochrome" | "monochromeansi" | "ansi" => Self::MonochromeANSI,
            _ => Self::DarkSlateCyan,
        }
    }
}

/// Strongly typed design tokens representing the visual theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeTokens {
    pub mode: ThemeMode,
    pub bg_canvas: Style,
    pub bg_surface: Style,
    pub bg_overlay: Style,
    pub border_default: Style,
    pub border_focused: Style,
    pub text_primary: Style,
    pub text_secondary: Style,
    pub text_muted: Style,
    pub accent_primary: Style,
    pub accent_secondary: Style,
    pub diff_addition: Style,
    pub diff_deletion: Style,

    // 12-Status semantic styles
    pub status_ok: Style,
    pub status_running: Style,
    pub status_waiting: Style,
    pub status_blocked: Style,
    pub status_failed: Style,
    pub status_warning: Style,
    pub status_asking: Style,
    pub status_denied: Style,
    pub status_planning: Style,
    pub status_verifying: Style,
    pub status_paused: Style,
    pub status_cancelled: Style,
}

impl ThemeTokens {
    /// Resolves theme tokens based on requested mode, automatically enforcing
    /// MonochromeANSI if the standard `NO_COLOR` environment variable is set.
    pub fn resolve(mode: ThemeMode) -> Self {
        let active_mode = if Self::is_no_color_active() {
            ThemeMode::MonochromeANSI
        } else {
            mode
        };

        match active_mode {
            ThemeMode::DarkSlateCyan => Self::dark_slate_cyan(),
            ThemeMode::HighContrast => Self::high_contrast(),
            ThemeMode::CleanLight => Self::clean_light(),
            ThemeMode::MonochromeANSI => Self::monochrome_ansi(),
        }
    }

    /// Checks whether the `NO_COLOR` environment variable is present and non-empty.
    pub fn is_no_color_active() -> bool {
        match std::env::var("NO_COLOR") {
            Ok(val) => !val.is_empty(),
            Err(_) => false,
        }
    }

    fn dark_slate_cyan() -> Self {
        Self {
            mode: ThemeMode::DarkSlateCyan,
            bg_canvas: Style::default().bg(Color::Rgb(15, 23, 42)),
            bg_surface: Style::default().bg(Color::Rgb(30, 41, 59)),
            bg_overlay: Style::default().bg(Color::Rgb(51, 65, 85)),
            border_default: Style::default().fg(Color::Rgb(71, 85, 105)),
            border_focused: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            text_primary: Style::default().fg(Color::Rgb(241, 245, 249)),
            text_secondary: Style::default().fg(Color::Rgb(203, 213, 225)),
            text_muted: Style::default().fg(Color::Rgb(148, 163, 184)),
            accent_primary: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default().fg(Color::LightCyan),
            diff_addition: Style::default().fg(Color::Green),
            diff_deletion: Style::default().fg(Color::Red),

            status_ok: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            status_running: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            status_waiting: Style::default().fg(Color::Yellow),
            status_blocked: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status_failed: Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
            status_warning: Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
            status_asking: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            status_denied: Style::default().fg(Color::Red),
            status_planning: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            status_verifying: Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
            status_paused: Style::default().fg(Color::Gray),
            status_cancelled: Style::default().fg(Color::DarkGray),
        }
    }

    fn high_contrast() -> Self {
        Self {
            mode: ThemeMode::HighContrast,
            bg_canvas: Style::default().bg(Color::Black),
            bg_surface: Style::default().bg(Color::Black),
            bg_overlay: Style::default().bg(Color::DarkGray),
            border_default: Style::default().fg(Color::White),
            border_focused: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            text_primary: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            text_secondary: Style::default().fg(Color::White),
            text_muted: Style::default().fg(Color::Gray),
            accent_primary: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default().fg(Color::LightYellow),
            diff_addition: Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
            diff_deletion: Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),

            status_ok: Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
            status_running: Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
            status_waiting: Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
            status_blocked: Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
            status_failed: Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
            status_warning: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            status_asking: Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
            status_denied: Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
            status_planning: Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
            status_verifying: Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
            status_paused: Style::default().fg(Color::White),
            status_cancelled: Style::default().fg(Color::Gray),
        }
    }

    fn clean_light() -> Self {
        Self {
            mode: ThemeMode::CleanLight,
            bg_canvas: Style::default().bg(Color::Rgb(248, 250, 252)),
            bg_surface: Style::default().bg(Color::Rgb(241, 245, 249)),
            bg_overlay: Style::default().bg(Color::Rgb(226, 232, 240)),
            border_default: Style::default().fg(Color::Rgb(203, 213, 225)),
            border_focused: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            text_primary: Style::default().fg(Color::Rgb(15, 23, 42)),
            text_secondary: Style::default().fg(Color::Rgb(51, 65, 85)),
            text_muted: Style::default().fg(Color::Rgb(100, 116, 139)),
            accent_primary: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default().fg(Color::LightBlue),
            diff_addition: Style::default().fg(Color::Green),
            diff_deletion: Style::default().fg(Color::Red),

            status_ok: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            status_running: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            status_waiting: Style::default().fg(Color::Rgb(180, 83, 9)),
            status_blocked: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status_failed: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status_warning: Style::default()
                .fg(Color::Rgb(180, 83, 9))
                .add_modifier(Modifier::BOLD),
            status_asking: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            status_denied: Style::default().fg(Color::Red),
            status_planning: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            status_verifying: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            status_paused: Style::default().fg(Color::DarkGray),
            status_cancelled: Style::default().fg(Color::Gray),
        }
    }

    fn monochrome_ansi() -> Self {
        Self {
            mode: ThemeMode::MonochromeANSI,
            bg_canvas: Style::default(),
            bg_surface: Style::default(),
            bg_overlay: Style::default(),
            border_default: Style::default(),
            border_focused: Style::default().add_modifier(Modifier::BOLD),
            text_primary: Style::default(),
            text_secondary: Style::default(),
            text_muted: Style::default().add_modifier(Modifier::DIM),
            accent_primary: Style::default().add_modifier(Modifier::BOLD),
            accent_secondary: Style::default(),
            diff_addition: Style::default(),
            diff_deletion: Style::default().add_modifier(Modifier::CROSSED_OUT),

            status_ok: Style::default().add_modifier(Modifier::BOLD),
            status_running: Style::default().add_modifier(Modifier::BOLD),
            status_waiting: Style::default(),
            status_blocked: Style::default().add_modifier(Modifier::REVERSED),
            status_failed: Style::default().add_modifier(Modifier::REVERSED),
            status_warning: Style::default().add_modifier(Modifier::BOLD),
            status_asking: Style::default().add_modifier(Modifier::BOLD),
            status_denied: Style::default().add_modifier(Modifier::REVERSED),
            status_planning: Style::default().add_modifier(Modifier::BOLD),
            status_verifying: Style::default().add_modifier(Modifier::BOLD),
            status_paused: Style::default().add_modifier(Modifier::DIM),
            status_cancelled: Style::default().add_modifier(Modifier::DIM),
        }
    }
}
