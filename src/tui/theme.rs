//! Centralized Theme & Design Token System (TDS-03, D-03).
//!
//! Provides semantic design tokens for the conversation-first M31A TUI.
//! All renderers must style through these tokens — never scatter literal
//! `Color::Cyan / Yellow / Magenta` through presentation code.
//!
//! The default theme ("Default" / M31A) is a restrained developer-oriented
//! palette: neutral canvas, subtle separators, muted text hierarchy, and a
//! single quiet accent. Status meaning is carried by symbol + text, never by
//! color alone.

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

/// Canonical theme options for M31A TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    /// Restrained conversation-first default. Calm, professional, quiet.
    #[default]
    Default,
    /// Legacy dark theme, restyled to the same calm philosophy.
    /// Kept for configuration backwards-compatibility.
    DarkSlateCyan,
    HighContrast,
    CleanLight,
    MonochromeANSI,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "M31A Default",
            Self::DarkSlateCyan => "Dark Slate Cyan",
            Self::HighContrast => "High Contrast",
            Self::CleanLight => "Clean Light",
            Self::MonochromeANSI => "Monochrome ANSI",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::Default => Self::DarkSlateCyan,
            Self::DarkSlateCyan => Self::HighContrast,
            Self::HighContrast => Self::CleanLight,
            Self::CleanLight => Self::MonochromeANSI,
            Self::MonochromeANSI => Self::Default,
        }
    }

    pub fn from_str_relaxed(s: &str) -> Self {
        match s.to_lowercase().replace(['-', '_'], "").as_str() {
            "highcontrast" => Self::HighContrast,
            "cleanlight" | "light" => Self::CleanLight,
            "monochrome" | "monochromeansi" | "ansi" => Self::MonochromeANSI,
            "default" | "m31a" | "m31adefault" | "slate" | "terminal" => Self::Default,
            "darkslatecyan" | "darkslate" | "cyber" => Self::DarkSlateCyan,
            _ => Self::Default,
        }
    }

    /// canonical config string round-tripping through `from_str_relaxed`.
    /// single authority for `AppConfig.tui.theme` persistence. The default
    /// theme spelling is owned by `config::canonical::DEFAULT_TUI_THEME`
    /// (matching the schema default); the remaining ids are theme
    /// vocabulary owned here.
    pub fn to_config_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::DarkSlateCyan => crate::config::canonical::DEFAULT_TUI_THEME,
            Self::HighContrast => "high-contrast",
            Self::CleanLight => "clean-light",
            Self::MonochromeANSI => "monochrome-ansi",
        }
    }
}

/// Strongly typed design tokens representing the visual theme.
///
/// Legacy fields (`bg_canvas`, `accent_primary`, `status_*`, …) are retained
/// for compatibility. New code should prefer the semantic aliases:
/// `canvas / surface / surface_subtle / accent / focus / success / warning /
/// error / info / separator / selection`.
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

    // 12-Status semantic styles (restrained: no bold, color supports meaning)
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

    // Semantic palette aliases (preferred for new code)
    pub canvas: Style,
    pub surface: Style,
    pub surface_subtle: Style,
    pub accent: Style,
    pub focus: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub info: Style,
    pub separator: Style,
    pub selection: Style,
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
            ThemeMode::Default => Self::m31a_default(),
            ThemeMode::DarkSlateCyan => Self::dark_slate_cyan_calm(),
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

    /// Whether this theme must avoid color semantics (mono / NO_COLOR).
    pub fn is_mono(&self) -> bool {
        self.mode == ThemeMode::MonochromeANSI || Self::is_no_color_active()
    }

    /// Restrained default theme: neutral canvas, subtle separators, single
    /// quiet accent, muted status colors without bold.
    fn m31a_default() -> Self {
        let bg_canvas = Style::default().bg(Color::Rgb(13, 17, 23));
        let bg_surface = Style::default().bg(Color::Rgb(22, 27, 34));
        let bg_overlay = Style::default().bg(Color::Rgb(28, 33, 40));
        let border_default = Style::default().fg(Color::Rgb(48, 54, 61));
        let border_focused = Style::default().fg(Color::Rgb(110, 122, 135));
        let text_primary = Style::default().fg(Color::Rgb(230, 237, 243));
        let text_secondary = Style::default().fg(Color::Rgb(174, 184, 193));
        let text_muted = Style::default().fg(Color::Rgb(125, 133, 144));
        let accent_primary = Style::default().fg(Color::Rgb(137, 159, 180));
        let accent_secondary = Style::default().fg(Color::Rgb(125, 133, 144));
        let diff_addition = Style::default().fg(Color::Rgb(63, 185, 80));
        let diff_deletion = Style::default().fg(Color::Rgb(200, 120, 115));

        let status_ok = Style::default().fg(Color::Rgb(63, 185, 80));
        let status_running = Style::default().fg(Color::Rgb(174, 184, 193));
        let status_waiting = Style::default().fg(Color::Rgb(174, 184, 193));
        let status_blocked = Style::default().fg(Color::Rgb(210, 153, 34));
        let status_failed = Style::default().fg(Color::Rgb(214, 124, 118));
        let status_warning = Style::default().fg(Color::Rgb(210, 153, 34));
        let status_asking = Style::default().fg(Color::Rgb(137, 159, 180));
        let status_denied = Style::default().fg(Color::Rgb(214, 124, 118));
        let status_planning = Style::default().fg(Color::Rgb(174, 184, 193));
        let status_verifying = Style::default().fg(Color::Rgb(63, 185, 80));
        let status_paused = Style::default().fg(Color::Rgb(125, 133, 144));
        let status_cancelled = Style::default().fg(Color::Rgb(125, 133, 144));

        let separator = Style::default().fg(Color::Rgb(48, 54, 61));
        let selection = Style::default().bg(Color::Rgb(33, 38, 45));
        Self {
            mode: ThemeMode::Default,
            bg_canvas,
            bg_surface,
            bg_overlay,
            border_default,
            border_focused,
            text_primary,
            text_secondary,
            text_muted,
            accent_primary,
            accent_secondary,
            diff_addition,
            diff_deletion,
            status_ok,
            status_running,
            status_waiting,
            status_blocked,
            status_failed,
            status_warning,
            status_asking,
            status_denied,
            status_planning,
            status_verifying,
            status_paused,
            status_cancelled,
            canvas: bg_canvas,
            surface: bg_surface,
            surface_subtle: Style::default().bg(Color::Rgb(18, 22, 29)),
            accent: accent_primary,
            focus: border_focused,
            success: status_ok,
            warning: status_warning,
            error: status_failed,
            info: status_asking,
            separator,
            selection,
        }
    }

    /// Legacy dark theme restyled to the calm philosophy: no neon cyan,
    /// no bold-everywhere, muted statuses. Functionally identical to Default
    /// with a marginally lighter canvas so existing configs keep working
    /// without the old hacker-cockpit look.
    fn dark_slate_cyan_calm() -> Self {
        let mut t = Self::m31a_default();
        t.mode = ThemeMode::DarkSlateCyan;
        t.bg_canvas = Style::default().bg(Color::Rgb(15, 23, 42));
        t.bg_surface = Style::default().bg(Color::Rgb(22, 32, 50));
        t.canvas = t.bg_canvas;
        t.surface = t.bg_surface;
        t
    }

    fn high_contrast() -> Self {
        let bg_canvas = Style::default().bg(Color::Black);
        let bg_surface = Style::default().bg(Color::Black);
        let bg_overlay = Style::default().bg(Color::DarkGray);
        let border_default = Style::default().fg(Color::White);
        let border_focused = Style::default().fg(Color::Yellow);
        let text_primary = Style::default().fg(Color::White);
        let text_secondary = Style::default().fg(Color::White);
        let text_muted = Style::default().fg(Color::Gray);
        let accent_primary = Style::default().fg(Color::Yellow);
        let accent_secondary = Style::default().fg(Color::LightYellow);
        let diff_addition = Style::default().fg(Color::LightGreen);
        let diff_deletion = Style::default().fg(Color::LightRed);

        let status_ok = Style::default().fg(Color::LightGreen);
        let status_running = Style::default().fg(Color::White);
        let status_waiting = Style::default().fg(Color::White);
        let status_blocked = Style::default().fg(Color::LightRed);
        let status_failed = Style::default().fg(Color::LightRed);
        let status_warning = Style::default().fg(Color::Yellow);
        let status_asking = Style::default().fg(Color::LightYellow);
        let status_denied = Style::default().fg(Color::LightRed);
        let status_planning = Style::default().fg(Color::White);
        let status_verifying = Style::default().fg(Color::LightGreen);
        let status_paused = Style::default().fg(Color::White);
        let status_cancelled = Style::default().fg(Color::Gray);
        Self {
            mode: ThemeMode::HighContrast,
            bg_canvas,
            bg_surface,
            bg_overlay,
            border_default,
            border_focused,
            text_primary,
            text_secondary,
            text_muted,
            accent_primary,
            accent_secondary,
            diff_addition,
            diff_deletion,
            status_ok,
            status_running,
            status_waiting,
            status_blocked,
            status_failed,
            status_warning,
            status_asking,
            status_denied,
            status_planning,
            status_verifying,
            status_paused,
            status_cancelled,
            canvas: bg_canvas,
            surface: bg_surface,
            surface_subtle: Style::default().bg(Color::Black),
            accent: accent_primary,
            focus: border_focused,
            success: status_ok,
            warning: status_warning,
            error: status_failed,
            info: status_asking,
            separator: border_default,
            selection: Style::default().bg(Color::DarkGray),
        }
    }

    fn clean_light() -> Self {
        let bg_canvas = Style::default().bg(Color::Rgb(248, 250, 252));
        let bg_surface = Style::default().bg(Color::Rgb(241, 245, 249));
        let bg_overlay = Style::default().bg(Color::Rgb(226, 232, 240));
        let border_default = Style::default().fg(Color::Rgb(203, 213, 225));
        let border_focused = Style::default().fg(Color::Rgb(100, 116, 139));
        let text_primary = Style::default().fg(Color::Rgb(15, 23, 42));
        let text_secondary = Style::default().fg(Color::Rgb(51, 65, 85));
        let text_muted = Style::default().fg(Color::Rgb(100, 116, 139));
        let accent_primary = Style::default().fg(Color::Rgb(51, 65, 85));
        let accent_secondary = Style::default().fg(Color::Rgb(100, 116, 139));
        let diff_addition = Style::default().fg(Color::Rgb(22, 128, 57));
        let diff_deletion = Style::default().fg(Color::Rgb(176, 32, 37));

        let status_ok = Style::default().fg(Color::Rgb(22, 128, 57));
        let status_running = Style::default().fg(Color::Rgb(51, 65, 85));
        let status_waiting = Style::default().fg(Color::Rgb(100, 116, 139));
        let status_blocked = Style::default().fg(Color::Rgb(154, 103, 0));
        let status_failed = Style::default().fg(Color::Rgb(176, 32, 37));
        let status_warning = Style::default().fg(Color::Rgb(154, 103, 0));
        let status_asking = Style::default().fg(Color::Rgb(51, 65, 85));
        let status_denied = Style::default().fg(Color::Rgb(176, 32, 37));
        let status_planning = Style::default().fg(Color::Rgb(51, 65, 85));
        let status_verifying = Style::default().fg(Color::Rgb(22, 128, 57));
        let status_paused = Style::default().fg(Color::Rgb(100, 116, 139));
        let status_cancelled = Style::default().fg(Color::Rgb(100, 116, 139));
        Self {
            mode: ThemeMode::CleanLight,
            bg_canvas,
            bg_surface,
            bg_overlay,
            border_default,
            border_focused,
            text_primary,
            text_secondary,
            text_muted,
            accent_primary,
            accent_secondary,
            diff_addition,
            diff_deletion,
            status_ok,
            status_running,
            status_waiting,
            status_blocked,
            status_failed,
            status_warning,
            status_asking,
            status_denied,
            status_planning,
            status_verifying,
            status_paused,
            status_cancelled,
            canvas: bg_canvas,
            surface: bg_surface,
            surface_subtle: Style::default().bg(Color::Rgb(241, 245, 249)),
            accent: accent_primary,
            focus: border_focused,
            success: status_ok,
            warning: status_warning,
            error: status_failed,
            info: status_asking,
            separator: border_default,
            selection: Style::default().bg(Color::Rgb(226, 232, 240)),
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
            status_running: Style::default(),
            status_waiting: Style::default(),
            status_blocked: Style::default().add_modifier(Modifier::REVERSED),
            status_failed: Style::default().add_modifier(Modifier::REVERSED),
            status_warning: Style::default().add_modifier(Modifier::BOLD),
            status_asking: Style::default().add_modifier(Modifier::BOLD),
            status_denied: Style::default().add_modifier(Modifier::REVERSED),
            status_planning: Style::default(),
            status_verifying: Style::default(),
            status_paused: Style::default().add_modifier(Modifier::DIM),
            status_cancelled: Style::default().add_modifier(Modifier::DIM),

            canvas: Style::default(),
            surface: Style::default(),
            surface_subtle: Style::default(),
            accent: Style::default().add_modifier(Modifier::BOLD),
            focus: Style::default().add_modifier(Modifier::BOLD),
            success: Style::default().add_modifier(Modifier::BOLD),
            warning: Style::default().add_modifier(Modifier::BOLD),
            error: Style::default().add_modifier(Modifier::REVERSED),
            info: Style::default(),
            separator: Style::default(),
            selection: Style::default().add_modifier(Modifier::REVERSED),
        }
    }
}
