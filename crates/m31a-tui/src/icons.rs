//! Centralized Lucide Icon System for M31A.
//!
//! Provides a compile-time-safe icon abstraction with three explicit rendering modes:
//! - `Lucide`: Official Lucide static icon font using pinned private-use/canonical codepoints.
//! - `Unicode`: Clean, universally supported non-emoji Unicode symbols.
//! - `Ascii`: Universally readable ASCII fallbacks for restricted terminals.
//!
//! # Font Setup Guide for Users
//!
//! To use the `Lucide` icon font in your terminal:
//! 1. Download the Lucide static font (`lucide.ttf` or Nerd Font variant with Lucide icons)
//!    from the official Lucide releases (<https://github.com/lucide-icons/lucide>).
//! 2. Install the font in your operating system's font book / font manager.
//! 3. Configure your terminal emulator (e.g. Alacritty, Kitty, WezTerm, iTerm2, Windows Terminal)
//!    to use the Lucide font or configure fallback font glyph coverage.
//! 4. In `~/.config/m31a/config.toml` (or `%APPDATA%\m31a\config.toml`), set:
//!    ```toml
//!    [tui]
//!    icons = "lucide" # Options: "lucide", "unicode", "ascii"
//!    ```
//!
//! If not explicitly set, M31A defaults to `unicode`, which renders clean, non-emoji
//! Unicode symbols without requiring special font installations.

use ratatui::style::Style;
use ratatui::text::Span;
use serde::{Deserialize, Serialize};
use std::fmt;
use unicode_width::UnicodeWidthStr;

/// Explicit rendering mode for icons across M31A interface components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconMode {
    /// Official Lucide icon font codepoints.
    Lucide,
    /// Clean, non-emoji Unicode glyphs (default).
    #[default]
    Unicode,
    /// Universally readable ASCII representations.
    Ascii,
}

impl IconMode {
    pub fn parse_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "lucide" => Self::Lucide,
            "ascii" => Self::Ascii,
            _ => Self::Unicode,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lucide => "lucide",
            Self::Unicode => "unicode",
            Self::Ascii => "ascii",
        }
    }
}

impl fmt::Display for IconMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Centralized, strongly typed registry of interface icons used by M31A.
///
/// Every M31A-owned interface element must retrieve glyphs through this enum
/// rather than scattering hardcoded glyphs or emojis across modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    // === Navigation ===
    MessageSquare,
    History,
    Cpu,
    Plug,
    Bot,
    Folder,
    GitBranch,
    GitCompare,
    Wrench,
    Settings,
    Activity,

    // === Workspace & Context ===
    FolderOpen,
    File,
    FileCode,
    FileText,
    FileJson,
    Terminal,
    Database,
    Search,
    Context,
    Chart,
    Shield,
    ShieldAlert,

    // === Execution & Status ===
    CircleCheck,
    CircleX,
    AlertTriangle,
    LoaderCircle,
    Play,
    Pause,
    Stop,
    Check,
    X,
    Clock,
    Code,
    Command,

    // === Additional Interface Glyphs ===
    Key,
    Lock,
    Sliders,
    Sparkles,
    Layers,
    ChevronRight,
    ArrowUpDown,
    RefreshCw,
    HelpCircle,
    Info,
    Server,
    Zap,
    User,
    Trash,
    Edit,
    Eye,
}

impl Icon {
    /// Returns the pinned Lucide icon font codepoint.
    pub const fn lucide_glyph(&self) -> &'static str {
        match self {
            // Navigation
            Self::MessageSquare => "\u{E1B0}",
            Self::History => "\u{E160}",
            Self::Cpu => "\u{E0C4}",
            Self::Plug => "\u{E208}",
            Self::Bot => "\u{E06C}",
            Self::Folder => "\u{E128}",
            Self::GitBranch => "\u{E143}",
            Self::GitCompare => "\u{E144}",
            Self::Wrench => "\u{E2B4}",
            Self::Settings => "\u{E248}",
            Self::Activity => "\u{E005}",

            // Workspace & Context
            Self::FolderOpen => "\u{E129}",
            Self::File => "\u{E10E}",
            Self::FileCode => "\u{E110}",
            Self::FileText => "\u{E119}",
            Self::FileJson => "\u{E114}",
            Self::Terminal => "\u{E27A}",
            Self::Database => "\u{E0D0}",
            Self::Search => "\u{E23A}",
            Self::Context => "\u{E185}",
            Self::Chart => "\u{E095}",
            Self::Shield => "\u{E24F}",
            Self::ShieldAlert => "\u{E250}",

            // Execution & Status
            Self::CircleCheck => "\u{E0A2}",
            Self::CircleX => "\u{E0AA}",
            Self::AlertTriangle => "\u{E01C}",
            Self::LoaderCircle => "\u{E198}",
            Self::Play => "\u{E206}",
            Self::Pause => "\u{E1FB}",
            Self::Stop => "\u{E264}",
            Self::Check => "\u{E09E}",
            Self::X => "\u{E2BD}",
            Self::Clock => "\u{E0B0}",
            Self::Code => "\u{E0B4}",
            Self::Command => "\u{E0BD}",

            // Additional Interface Glyphs
            Self::Key => "\u{E178}",
            Self::Lock => "\u{E18C}",
            Self::Sliders => "\u{E258}",
            Self::Sparkles => "\u{E25E}",
            Self::Layers => "\u{E185}",
            Self::ChevronRight => "\u{E09B}",
            Self::ArrowUpDown => "\u{E046}",
            Self::RefreshCw => "\u{E223}",
            Self::HelpCircle => "\u{E15E}",
            Self::Info => "\u{E171}",
            Self::Server => "\u{E243}",
            Self::Zap => "\u{E2C0}",
            Self::User => "\u{E285}",
            Self::Trash => "\u{E280}",
            Self::Edit => "\u{E101}",
            Self::Eye => "\u{E108}",
        }
    }

    /// Returns the clean non-emoji Unicode glyph.
    pub const fn unicode_glyph(&self) -> &'static str {
        match self {
            // Navigation
            Self::MessageSquare => "▪",
            Self::History => "↺",
            Self::Cpu => "⊞",
            Self::Plug => "⚡",
            Self::Bot => "◇",
            Self::Folder => "▸",
            Self::GitBranch => "⎇",
            Self::GitCompare => "⇄",
            Self::Wrench => "⚒",
            Self::Settings => "⚙",
            Self::Activity => "~",

            // Workspace & Context
            Self::FolderOpen => "▾",
            Self::File => "·",
            Self::FileCode => "⟨/⟩",
            Self::FileText => "≡",
            Self::FileJson => "{}",
            Self::Terminal => "❯_",
            Self::Database => "⛁",
            Self::Search => "⚲",
            Self::Context => "§",
            Self::Chart => "☷",
            Self::Shield => "◈",
            Self::ShieldAlert => "⛉",

            // Execution & Status
            Self::CircleCheck => "✔",
            Self::CircleX => "✘",
            Self::AlertTriangle => "▲",
            Self::LoaderCircle => "◐",
            Self::Play => "▶",
            Self::Pause => "❚❚",
            Self::Stop => "■",
            Self::Check => "✓",
            Self::X => "✗",
            Self::Clock => "◷",
            Self::Code => "⟨/⟩",
            Self::Command => "⌘",

            // Additional Interface Glyphs
            Self::Key => "⚿",
            Self::Lock => "⚿",
            Self::Sliders => "≎",
            Self::Sparkles => "✧",
            Self::Layers => "☰",
            Self::ChevronRight => "›",
            Self::ArrowUpDown => "↕",
            Self::RefreshCw => "↺",
            Self::HelpCircle => "?",
            Self::Info => "ℹ",
            Self::Server => "⌸",
            Self::Zap => "⚡",
            Self::User => "👤",
            Self::Trash => "✕",
            Self::Edit => "✎",
            Self::Eye => "☉",
        }
    }

    /// Returns the universally readable ASCII fallback.
    pub const fn ascii_glyph(&self) -> &'static str {
        match self {
            // Navigation
            Self::MessageSquare => "[#]",
            Self::History => "(h)",
            Self::Cpu => "[CPU]",
            Self::Plug => "[+]",
            Self::Bot => "[A]",
            Self::Folder => "[D]",
            Self::GitBranch => "|",
            Self::GitCompare => "<>",
            Self::Wrench => "[W]",
            Self::Settings => "[*]",
            Self::Activity => "~",

            // Workspace & Context
            Self::FolderOpen => "[O]",
            Self::File => "[F]",
            Self::FileCode => "</>",
            Self::FileText => "[T]",
            Self::FileJson => "{}",
            Self::Terminal => ">",
            Self::Database => "[DB]",
            Self::Search => "?",
            Self::Context => "[C]",
            Self::Chart => "[#]",
            Self::Shield => "[S]",
            Self::ShieldAlert => "[!S]",

            // Execution & Status
            Self::CircleCheck => "[v]",
            Self::CircleX => "[x]",
            Self::AlertTriangle => "[!]",
            Self::LoaderCircle => "*",
            Self::Play => ">",
            Self::Pause => "||",
            Self::Stop => "[]",
            Self::Check => "v",
            Self::X => "x",
            Self::Clock => "[t]",
            Self::Code => "<>",
            Self::Command => "Cmd",

            // Additional Interface Glyphs
            Self::Key => "[K]",
            Self::Lock => "[L]",
            Self::Sliders => "[=]",
            Self::Sparkles => "*",
            Self::Layers => "[=]",
            Self::ChevronRight => ">",
            Self::ArrowUpDown => "^v",
            Self::RefreshCw => "(R)",
            Self::HelpCircle => "(?)",
            Self::Info => "(i)",
            Self::Server => "[S]",
            Self::Zap => "*",
            Self::User => "[U]",
            Self::Trash => "[X]",
            Self::Edit => "[E]",
            Self::Eye => "[O]",
        }
    }

    /// Returns the glyph corresponding to the given [`IconMode`].
    pub fn glyph(&self, mode: IconMode) -> &'static str {
        match mode {
            IconMode::Lucide => self.lucide_glyph(),
            IconMode::Unicode => self.unicode_glyph(),
            IconMode::Ascii => self.ascii_glyph(),
        }
    }

    /// Renders the icon respecting the active theme's typography configuration
    /// (`ThemeTypography.use_icons`), `Theme.icon_mode`, and user override.
    pub fn render(&self, theme: &crate::theme::Theme) -> &'static str {
        let mode = crate::theme::effective_icon_mode(theme);
        self.glyph(mode)
    }

    /// Returns the terminal display width in columns for this icon under the given mode.
    pub fn width(&self, mode: IconMode) -> usize {
        UnicodeWidthStr::width(self.glyph(mode))
    }

    /// Convenience helper to create a styled [`Span`] with this icon.
    pub fn styled_span<'a>(&self, theme: &crate::theme::Theme, style: Style) -> Span<'a> {
        Span::styled(self.render(theme), style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_icons_have_non_empty_glyphs_in_all_modes() {
        let all_icons = [
            Icon::MessageSquare,
            Icon::History,
            Icon::Cpu,
            Icon::Plug,
            Icon::Bot,
            Icon::Folder,
            Icon::GitBranch,
            Icon::GitCompare,
            Icon::Wrench,
            Icon::Settings,
            Icon::Activity,
            Icon::FolderOpen,
            Icon::File,
            Icon::FileCode,
            Icon::FileText,
            Icon::FileJson,
            Icon::Terminal,
            Icon::Database,
            Icon::Search,
            Icon::Context,
            Icon::Chart,
            Icon::Shield,
            Icon::ShieldAlert,
            Icon::CircleCheck,
            Icon::CircleX,
            Icon::AlertTriangle,
            Icon::LoaderCircle,
            Icon::Play,
            Icon::Pause,
            Icon::Stop,
            Icon::Check,
            Icon::X,
            Icon::Clock,
            Icon::Code,
            Icon::Command,
            Icon::Key,
            Icon::Lock,
            Icon::Sliders,
            Icon::Sparkles,
            Icon::Layers,
            Icon::ChevronRight,
            Icon::ArrowUpDown,
            Icon::RefreshCw,
            Icon::HelpCircle,
            Icon::Info,
            Icon::Server,
            Icon::Zap,
            Icon::User,
            Icon::Trash,
            Icon::Edit,
            Icon::Eye,
        ];

        for icon in all_icons {
            assert!(
                !icon.lucide_glyph().is_empty(),
                "Lucide glyph for {:?} is empty",
                icon
            );
            assert!(
                !icon.unicode_glyph().is_empty(),
                "Unicode glyph for {:?} is empty",
                icon
            );
            assert!(
                !icon.ascii_glyph().is_empty(),
                "ASCII glyph for {:?} is empty",
                icon
            );

            assert!(icon.width(IconMode::Lucide) >= 1);
            assert!(icon.width(IconMode::Unicode) >= 1);
            assert!(icon.width(IconMode::Ascii) >= 1);
        }
    }

    #[test]
    fn test_icon_mode_parsing() {
        assert_eq!(IconMode::parse_str("lucide"), IconMode::Lucide);
        assert_eq!(IconMode::parse_str("LUCIDE"), IconMode::Lucide);
        assert_eq!(IconMode::parse_str("unicode"), IconMode::Unicode);
        assert_eq!(IconMode::parse_str("ascii"), IconMode::Ascii);
        assert_eq!(IconMode::parse_str("unknown"), IconMode::Unicode);
    }

    #[test]
    fn test_theme_use_icons_fallback() {
        let mut dark = crate::theme::Theme::dark();
        assert_eq!(
            Icon::Command.render(&dark),
            Icon::Command.glyph(dark.icon_mode)
        );

        // When use_icons is disabled, ASCII fallback is strictly used
        dark.typography.use_icons = false;
        assert_eq!(Icon::Command.render(&dark), Icon::Command.ascii_glyph());
    }
}
