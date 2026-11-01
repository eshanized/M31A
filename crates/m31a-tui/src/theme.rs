//! Professional TUI Theme System
//!
//! Clean, minimal, developer-focused design — no sci-fi, no cyberpunk, no neon.
//! Developer-focused theme with neutral backgrounds, clear syntax tokens, and high contrast.

use ratatui::style::{Color, Modifier, Style};

/// Semantic color roles — never hardcode raw colors in widgets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorRole {
    // Background layers (elevated surfaces)
    Surface0, // Base background
    Surface1, // Cards, panels, modals
    Surface2, // Hover/active states
    Surface3, // Highest elevation (tooltips, dropdowns)

    // Content hierarchy
    TextPrimary,   // Main body text
    TextSecondary, // Muted text, descriptions, line numbers
    TextMuted,     // Placeholders, disabled, hints
    TextInverse,   // Text on colored backgrounds

    // Accent / Brand (single accent color, used sparingly)
    Accent,      // Primary actions, focus, selection
    AccentMuted, // Subtle accent backgrounds

    // State colors (semantic, not decorative)
    Success, // Green - success, added lines, running
    Warning, // Amber - warnings, pending, modified
    Error,   // Red - errors, deleted lines, stopped
    Info,    // Blue - info, links, current branch

    // Borders & Dividers
    Border,       // Default border
    BorderFocus,  // Focused element border
    BorderSubtle, // Subtle dividers

    // Code / Syntax (minimal set)
    CodeKeyword,
    CodeString,
    CodeNumber,
    CodeComment,
    CodeFunction,
    CodeType,
}

/// Professional theme variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeVariant {
    /// Everblush dark theme — official palette (https://github.com/everblush)
    Everblush,
    /// Dark theme — alias for Everblush
    Dark,
    /// Light theme — for bright environments
    Light,
    /// High contrast — accessibility, low vision
    HighContrast,
}

/// Complete theme definition
#[derive(Debug, Clone)]
pub struct Theme {
    pub variant: ThemeVariant,
    pub colors: ThemeColors,
    pub spacing: ThemeSpacing,
    pub border: ThemeBorder,
    pub typography: ThemeTypography,
}

#[derive(Debug, Clone)]
pub struct ThemeColors {
    // Surface elevation
    pub surface_0: Color,
    pub surface_1: Color,
    pub surface_2: Color,
    pub surface_3: Color,

    // Content
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_inverse: Color,

    // Accent (single brand color)
    pub accent: Color,
    pub accent_muted: Color,

    // Semantic states
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Borders
    pub border: Color,
    pub border_focus: Color,
    pub border_subtle: Color,

    // Code syntax (subtle)
    pub code_keyword: Color,
    pub code_string: Color,
    pub code_number: Color,
    pub code_comment: Color,
    pub code_function: Color,
    pub code_type: Color,
}

#[derive(Debug, Clone)]
pub struct ThemeSpacing {
    pub padding_x: u16,
    pub padding_y: u16,
    pub gap: u16,
    pub border_radius: BorderRadius,
}

#[derive(Debug, Clone, Copy)]
pub enum BorderRadius {
    None,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone)]
pub struct ThemeBorder {
    pub default: ratatui::widgets::BorderType,
    pub focused: ratatui::widgets::BorderType,
    pub modal: ratatui::widgets::BorderType,
}

#[derive(Debug, Clone)]
pub struct ThemeTypography {
    pub header_bold: bool,
    pub use_icons: bool,
}

/// Theme implementations
impl Theme {
    /// Official Everblush theme palette (https://github.com/everblush)
    pub fn everblush() -> Self {
        Self {
            variant: ThemeVariant::Everblush,
            colors: ThemeColors {
                // Surfaces — Everblush deep blue-grays
                surface_0: Color::Rgb(0x14, 0x1b, 0x1e), // #141b1e - base background
                surface_1: Color::Rgb(0x23, 0x2a, 0x2d), // #232a2d - lighter background / panels
                surface_2: Color::Rgb(0x2d, 0x34, 0x37), // #2d3437 - hover / bright black
                surface_3: Color::Rgb(0x33, 0x3b, 0x3e), // #333b3e - dropdowns / elevated surfaces

                // Content — Everblush foreground text & gray hierarchy
                text_primary: Color::Rgb(0xda, 0xda, 0xda), // #dadada - foreground text
                text_secondary: Color::Rgb(0xb3, 0xb9, 0xb8), // #b3b9b8 - light gray
                text_muted: Color::Rgb(0x67, 0x6e, 0x71),   // #676e71 - comment / muted
                text_inverse: Color::Rgb(0x14, 0x1b, 0x1e), // #141b1e - text on colored background

                // Accent — Everblush signature Cyan (#6cbfbf)
                accent: Color::Rgb(0x6c, 0xbf, 0xbf), // #6cbfbf - Everblush cyan
                accent_muted: Color::Rgb(0x1c, 0x33, 0x36), // #1c3336 - muted cyan background

                // States — Everblush semantic palette
                success: Color::Rgb(0x8c, 0xcf, 0x7e), // #8ccf7e - Everblush green
                warning: Color::Rgb(0xe5, 0xc7, 0x6b), // #e5c76b - Everblush yellow
                error: Color::Rgb(0xe5, 0x74, 0x74),   // #e57474 - Everblush red
                info: Color::Rgb(0x67, 0xb0, 0xe8),    // #67b0e8 - Everblush blue

                // Borders
                border: Color::Rgb(0x2e, 0x35, 0x38), // #2e3538 - subtle border
                border_focus: Color::Rgb(0x6c, 0xbf, 0xbf), // #6cbfbf - Everblush cyan focus
                border_subtle: Color::Rgb(0x23, 0x2a, 0x2d), // #232a2d

                // Code syntax — Everblush token styling
                code_keyword: Color::Rgb(0xc4, 0x7f, 0xd5), // #c47fd5 - Everblush magenta
                code_string: Color::Rgb(0x8c, 0xcf, 0x7e),  // #8ccf7e - Everblush green
                code_number: Color::Rgb(0xe5, 0xc7, 0x6b),  // #e5c76b - Everblush yellow
                code_comment: Color::Rgb(0xb3, 0xb9, 0xb8), // #b3b9b8 - Everblush light gray
                code_function: Color::Rgb(0x67, 0xb0, 0xe8), // #67b0e8 - Everblush blue
                code_type: Color::Rgb(0x6c, 0xbf, 0xbf),    // #6cbfbf - Everblush cyan
            },
            spacing: ThemeSpacing {
                padding_x: 2,
                padding_y: 1,
                gap: 1,
                border_radius: BorderRadius::Medium,
            },
            border: ThemeBorder {
                default: ratatui::widgets::BorderType::Rounded,
                focused: ratatui::widgets::BorderType::Rounded,
                modal: ratatui::widgets::BorderType::Double,
            },
            typography: ThemeTypography {
                header_bold: true,
                use_icons: true,
            },
        }
    }

    /// Default dark theme — uses Everblush palette
    pub fn dark() -> Self {
        Self::everblush()
    }

    /// Light theme — clean, paper-like
    pub fn light() -> Self {
        Self {
            variant: ThemeVariant::Light,
            colors: ThemeColors {
                surface_0: Color::Rgb(0xfa, 0xfa, 0xfa), // #fafafa
                surface_1: Color::Rgb(0xff, 0xff, 0xff), // #ffffff
                surface_2: Color::Rgb(0xf0, 0xf0, 0xf0), // #f0f0f0
                surface_3: Color::Rgb(0xe8, 0xe8, 0xe8), // #e8e8e8

                text_primary: Color::Rgb(0x1a, 0x1a, 0x1a), // #1a1a1a
                text_secondary: Color::Rgb(0x52, 0x52, 0x52), // #525252
                text_muted: Color::Rgb(0x9e, 0x9e, 0x9e),   // #9e9e9e
                text_inverse: Color::Rgb(0xff, 0xff, 0xff),

                accent: Color::Rgb(0x25, 0x63, 0xeb), // #2563eb - blue-600
                accent_muted: Color::Rgb(0xdb, 0xea, 0xfe), // #dbeafe - blue-100

                success: Color::Rgb(0x16, 0xa3, 0x4a), // #16a34a
                warning: Color::Rgb(0xd9, 0x77, 0x06), // #d97706
                error: Color::Rgb(0xdc, 0x26, 0x26),   // #dc2626
                info: Color::Rgb(0x08, 0x91, 0xb2),    // #0891b2

                border: Color::Rgb(0xe0, 0xe0, 0xe0),
                border_focus: Color::Rgb(0x25, 0x63, 0xeb),
                border_subtle: Color::Rgb(0xf0, 0xf0, 0xf0),

                code_keyword: Color::Rgb(0x93, 0x33, 0xea),
                code_string: Color::Rgb(0x16, 0xa3, 0x4a),
                code_number: Color::Rgb(0xea, 0x58, 0x0c),
                code_comment: Color::Rgb(0x94, 0xa3, 0xb8),
                code_function: Color::Rgb(0x08, 0x91, 0xb2),
                code_type: Color::Rgb(0xdb, 0x27, 0x77),
            },
            spacing: ThemeSpacing {
                padding_x: 2,
                padding_y: 1,
                gap: 1,
                border_radius: BorderRadius::Medium,
            },
            border: ThemeBorder {
                default: ratatui::widgets::BorderType::Rounded,
                focused: ratatui::widgets::BorderType::Rounded,
                modal: ratatui::widgets::BorderType::Double,
            },
            typography: ThemeTypography {
                header_bold: true,
                use_icons: true,
            },
        }
    }

    /// High contrast — WCAG AAA compliant
    pub fn high_contrast() -> Self {
        Self {
            variant: ThemeVariant::HighContrast,
            colors: ThemeColors {
                surface_0: Color::Black,
                surface_1: Color::Rgb(0x1a, 0x1a, 0x1a),
                surface_2: Color::Rgb(0x33, 0x33, 0x33),
                surface_3: Color::Rgb(0x4d, 0x4d, 0x4d),

                text_primary: Color::White,
                text_secondary: Color::Rgb(0xcc, 0xcc, 0xcc),
                text_muted: Color::Rgb(0x99, 0x99, 0x99),
                text_inverse: Color::Black,

                accent: Color::Rgb(0x00, 0x96, 0xff), // Bright blue
                accent_muted: Color::Rgb(0x00, 0x3d, 0x66),

                success: Color::Rgb(0x00, 0xff, 0x00), // Pure green
                warning: Color::Rgb(0xff, 0xcc, 0x00), // Bright yellow
                error: Color::Rgb(0xff, 0x33, 0x33),   // Bright red
                info: Color::Rgb(0x00, 0xcc, 0xff),    // Bright cyan

                border: Color::Rgb(0x66, 0x66, 0x66),
                border_focus: Color::Rgb(0x00, 0x96, 0xff),
                border_subtle: Color::Rgb(0x33, 0x33, 0x33),

                code_keyword: Color::Rgb(0xcc, 0x66, 0xff),
                code_string: Color::Rgb(0x66, 0xff, 0x66),
                code_number: Color::Rgb(0xff, 0xaa, 0x00),
                code_comment: Color::Rgb(0x99, 0x99, 0x99),
                code_function: Color::Rgb(0x00, 0xcc, 0xff),
                code_type: Color::Rgb(0xff, 0x66, 0xcc),
            },
            spacing: ThemeSpacing {
                padding_x: 2,
                padding_y: 1,
                gap: 1,
                border_radius: BorderRadius::None, // Sharp for high contrast
            },
            border: ThemeBorder {
                default: ratatui::widgets::BorderType::Plain,
                focused: ratatui::widgets::BorderType::Thick,
                modal: ratatui::widgets::BorderType::Thick,
            },
            typography: ThemeTypography {
                header_bold: true,
                use_icons: false, // Icons may not render in high contrast
            },
        }
    }

    /// Get color for semantic role
    pub fn color(&self, role: ColorRole) -> Color {
        match role {
            ColorRole::Surface0 => self.colors.surface_0,
            ColorRole::Surface1 => self.colors.surface_1,
            ColorRole::Surface2 => self.colors.surface_2,
            ColorRole::Surface3 => self.colors.surface_3,
            ColorRole::TextPrimary => self.colors.text_primary,
            ColorRole::TextSecondary => self.colors.text_secondary,
            ColorRole::TextMuted => self.colors.text_muted,
            ColorRole::TextInverse => self.colors.text_inverse,
            ColorRole::Accent => self.colors.accent,
            ColorRole::AccentMuted => self.colors.accent_muted,
            ColorRole::Success => self.colors.success,
            ColorRole::Warning => self.colors.warning,
            ColorRole::Error => self.colors.error,
            ColorRole::Info => self.colors.info,
            ColorRole::Border => self.colors.border,
            ColorRole::BorderFocus => self.colors.border_focus,
            ColorRole::BorderSubtle => self.colors.border_subtle,
            ColorRole::CodeKeyword => self.colors.code_keyword,
            ColorRole::CodeString => self.colors.code_string,
            ColorRole::CodeNumber => self.colors.code_number,
            ColorRole::CodeComment => self.colors.code_comment,
            ColorRole::CodeFunction => self.colors.code_function,
            ColorRole::CodeType => self.colors.code_type,
        }
    }

    /// Style helpers for common patterns
    pub fn style_primary(&self) -> Style {
        Style::default().fg(self.color(ColorRole::TextPrimary))
    }

    pub fn style_secondary(&self) -> Style {
        Style::default().fg(self.color(ColorRole::TextSecondary))
    }

    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.color(ColorRole::TextMuted))
    }

    pub fn style_accent(&self, bold: bool) -> Style {
        let mut s = Style::default().fg(self.color(ColorRole::Accent));
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    pub fn style_success(&self, bold: bool) -> Style {
        let mut s = Style::default().fg(self.color(ColorRole::Success));
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    pub fn style_warning(&self, bold: bool) -> Style {
        let mut s = Style::default().fg(self.color(ColorRole::Warning));
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    pub fn style_error(&self, bold: bool) -> Style {
        let mut s = Style::default().fg(self.color(ColorRole::Error));
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    pub fn style_info(&self, bold: bool) -> Style {
        let mut s = Style::default().fg(self.color(ColorRole::Info));
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    /// Surface backgrounds
    pub fn style_surface0(&self) -> Style {
        Style::default().bg(self.color(ColorRole::Surface0))
    }

    pub fn style_surface1(&self) -> Style {
        Style::default().bg(self.color(ColorRole::Surface1))
    }

    pub fn style_surface2(&self) -> Style {
        Style::default().bg(self.color(ColorRole::Surface2))
    }

    /// Border styles
    pub fn border_default(&self) -> Style {
        Style::default().fg(self.color(ColorRole::Border))
    }

    pub fn border_focus(&self) -> Style {
        Style::default().fg(self.color(ColorRole::BorderFocus))
    }

    pub fn border_subtle(&self) -> Style {
        Style::default().fg(self.color(ColorRole::BorderSubtle))
    }

    /// Selected/focused item style
    pub fn style_selected(&self) -> Style {
        Style::default()
            .fg(self.color(ColorRole::TextInverse))
            .bg(self.color(ColorRole::Accent))
            .add_modifier(Modifier::BOLD)
    }

    /// Subtle selected (for lists with focus but not active)
    pub fn style_selected_subtle(&self) -> Style {
        Style::default()
            .fg(self.color(ColorRole::TextPrimary))
            .bg(self.color(ColorRole::Surface2))
    }

    /// Muted addition background for diffs (Everblush dark moss for dark, pastel for light)
    pub fn diff_add_bg(&self) -> Color {
        match self.variant {
            ThemeVariant::Everblush | ThemeVariant::Dark => Color::Rgb(0x1a, 0x30, 0x24), // #1a3024 Everblush dark moss
            ThemeVariant::Light => Color::Rgb(218, 251, 225), // #DAFBE1 GitHub pastel green
            ThemeVariant::HighContrast => Color::Rgb(0, 50, 0),
        }
    }

    /// Muted deletion background for diffs (Everblush dark wine for dark, pastel for light)
    pub fn diff_del_bg(&self) -> Color {
        match self.variant {
            ThemeVariant::Everblush | ThemeVariant::Dark => Color::Rgb(0x3b, 0x1f, 0x22), // #3b1f22 Everblush dark wine
            ThemeVariant::Light => Color::Rgb(255, 235, 233), // #FFEBE9 GitHub pastel red
            ThemeVariant::HighContrast => Color::Rgb(60, 0, 0),
        }
    }
}

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

static EVERBLUSH_THEME: LazyLock<Theme> = LazyLock::new(Theme::everblush);
static LIGHT_THEME: LazyLock<Theme> = LazyLock::new(Theme::light);
static HC_THEME: LazyLock<Theme> = LazyLock::new(Theme::high_contrast);
static THEME_VARIANT: AtomicU8 = AtomicU8::new(0);

/// Initialize or switch global theme from config or command
pub fn init_theme(variant: ThemeVariant) {
    let val = match variant {
        ThemeVariant::Everblush | ThemeVariant::Dark => 0,
        ThemeVariant::Light => 1,
        ThemeVariant::HighContrast => 2,
    };
    THEME_VARIANT.store(val, Ordering::SeqCst);
}

/// Get current theme
pub fn theme() -> &'static Theme {
    match THEME_VARIANT.load(Ordering::Relaxed) {
        1 => &LIGHT_THEME,
        2 => &HC_THEME,
        _ => &EVERBLUSH_THEME,
    }
}

/// Convenience macro for theme colors
#[macro_export]
macro_rules! tcolor {
    ($role:expr) => {
        $crate::theme::theme().color($role)
    };
}

#[macro_export]
macro_rules! tstyle {
    (primary) => {
        $crate::theme::theme().style_primary()
    };
    (secondary) => {
        $crate::theme::theme().style_secondary()
    };
    (muted) => {
        $crate::theme::theme().style_muted()
    };
    (accent) => {
        $crate::theme::theme().style_accent(true)
    };
    (accent_plain) => {
        $crate::theme::theme().style_accent(false)
    };
    (success) => {
        $crate::theme::theme().style_success(true)
    };
    (warning) => {
        $crate::theme::theme().style_warning(true)
    };
    (error) => {
        $crate::theme::theme().style_error(true)
    };
    (info) => {
        $crate::theme::theme().style_info(true)
    };
    (selected) => {
        $crate::theme::theme().style_selected()
    };
    (selected_subtle) => {
        $crate::theme::theme().style_selected_subtle()
    };
    (surface0) => {
        $crate::theme::theme().style_surface0()
    };
    (surface1) => {
        $crate::theme::theme().style_surface1()
    };
    (surface2) => {
        $crate::theme::theme().style_surface2()
    };
    (border) => {
        $crate::theme::theme().border_default()
    };
    (border_focus) => {
        $crate::theme::theme().border_focus()
    };
    (border_subtle) => {
        $crate::theme::theme().border_subtle()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_everblush_palette_specification() {
        let eb = Theme::everblush();
        // Official Everblush hex values
        assert_eq!(eb.colors.surface_0, Color::Rgb(0x14, 0x1b, 0x1e)); // #141b1e background
        assert_eq!(eb.colors.surface_1, Color::Rgb(0x23, 0x2a, 0x2d)); // #232a2d lighter background
        assert_eq!(eb.colors.text_primary, Color::Rgb(0xda, 0xda, 0xda)); // #dadada foreground
        assert_eq!(eb.colors.text_secondary, Color::Rgb(0xb3, 0xb9, 0xb8)); // #b3b9b8 light gray
        assert_eq!(eb.colors.accent, Color::Rgb(0x6c, 0xbf, 0xbf)); // #6cbfbf cyan accent
        assert_eq!(eb.colors.success, Color::Rgb(0x8c, 0xcf, 0x7e)); // #8ccf7e green
        assert_eq!(eb.colors.warning, Color::Rgb(0xe5, 0xc7, 0x6b)); // #e5c76b yellow
        assert_eq!(eb.colors.error, Color::Rgb(0xe5, 0x74, 0x74)); // #e57474 red
        assert_eq!(eb.colors.info, Color::Rgb(0x67, 0xb0, 0xe8)); // #67b0e8 blue
        assert_eq!(eb.colors.code_keyword, Color::Rgb(0xc4, 0x7f, 0xd5)); // #c47fd5 magenta
    }

    #[test]
    fn test_everblush_diff_washes() {
        let eb = Theme::everblush();
        assert_eq!(eb.diff_add_bg(), Color::Rgb(0x1a, 0x30, 0x24));
        assert_eq!(eb.diff_del_bg(), Color::Rgb(0x3b, 0x1f, 0x22));
    }

    #[test]
    fn test_init_theme_everblush_switching() {
        init_theme(ThemeVariant::Everblush);
        let curr = theme();
        assert_eq!(curr.colors.surface_0, Color::Rgb(0x14, 0x1b, 0x1e));
        assert_eq!(curr.colors.accent, Color::Rgb(0x6c, 0xbf, 0xbf));
    }
}
