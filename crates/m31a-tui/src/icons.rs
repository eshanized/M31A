//! Centralized Lucide Icon System for M31A.
//!
//! Provides a compile-time-safe icon abstraction with three explicit rendering modes:
//! - `Lucide`: Official Lucide static icon font using pinned private-use/canonical codepoints
//!   validated against official release 0.460.0 (`lucide-font-0.460.0`).
//! - `Unicode`: Clean, universally supported non-emoji Unicode symbols.
//! - `Ascii`: Universally readable ASCII fallbacks for restricted terminals.
//!
//! # Font Setup Guide for Users
//!
//! To use the `Lucide` icon font in your terminal:
//! 1. Download the Lucide static font (`lucide.ttf` or Nerd Font variant with Lucide icons)
//!    from the official Lucide releases (<https://github.com/lucide-icons/lucide/releases/tag/0.460.0>).
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
    /// Returns the official icon name in the pinned `lucide-font-0.460.0` manifest.
    pub const fn lucide_icon_name(&self) -> &'static str {
        match self {
            // Navigation
            Self::MessageSquare => "message-square",
            Self::History => "history",
            Self::Cpu => "cpu",
            Self::Plug => "plug",
            Self::Bot => "bot",
            Self::Folder => "folder",
            Self::GitBranch => "git-branch",
            Self::GitCompare => "git-compare",
            Self::Wrench => "wrench",
            Self::Settings => "settings",
            Self::Activity => "activity",

            // Workspace & Context
            Self::FolderOpen => "folder-open",
            Self::File => "file",
            Self::FileCode => "file-code",
            Self::FileText => "file-text",
            Self::FileJson => "file-json",
            Self::Terminal => "terminal",
            Self::Database => "database",
            Self::Search => "search",
            Self::Context => "boxes",
            Self::Chart => "chart-bar",
            Self::Shield => "shield",
            Self::ShieldAlert => "shield-alert",

            // Execution & Status
            Self::CircleCheck => "circle-check",
            Self::CircleX => "circle-x",
            Self::AlertTriangle => "triangle-alert",
            Self::LoaderCircle => "loader-circle",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "square",
            Self::Check => "check",
            Self::X => "x",
            Self::Clock => "clock",
            Self::Code => "code",
            Self::Command => "command",

            // Additional Interface Glyphs
            Self::Key => "key",
            Self::Lock => "lock",
            Self::Sliders => "sliders-horizontal",
            Self::Sparkles => "sparkles",
            Self::Layers => "layers",
            Self::ChevronRight => "chevron-right",
            Self::ArrowUpDown => "arrow-up-down",
            Self::RefreshCw => "refresh-cw",
            Self::HelpCircle => "circle-help",
            Self::Info => "info",
            Self::Server => "server",
            Self::Zap => "zap",
            Self::User => "user",
            Self::Trash => "trash-2",
            Self::Edit => "square-pen",
            Self::Eye => "eye",
        }
    }

    /// Returns the pinned Lucide icon font codepoint (validated against official release 0.460.0).
    pub const fn lucide_glyph(&self) -> &'static str {
        match self {
            // Navigation
            Self::MessageSquare => "\u{E11B}",
            Self::History => "\u{E1F5}",
            Self::Cpu => "\u{E0AD}",
            Self::Plug => "\u{E383}",
            Self::Bot => "\u{E1BB}",
            Self::Folder => "\u{E0DC}",
            Self::GitBranch => "\u{E0E5}",
            Self::GitCompare => "\u{E35D}",
            Self::Wrench => "\u{E1B1}",
            Self::Settings => "\u{E158}",
            Self::Activity => "\u{E038}",

            // Workspace & Context
            Self::FolderOpen => "\u{E247}",
            Self::File => "\u{E0C4}",
            Self::FileCode => "\u{E0C7}",
            Self::FileText => "\u{E0D0}",
            Self::FileJson => "\u{E36F}",
            Self::Terminal => "\u{E185}",
            Self::Database => "\u{E0B1}",
            Self::Search => "\u{E155}",
            Self::Context => "\u{E2D0}",
            Self::Chart => "\u{E2A2}",
            Self::Shield => "\u{E15C}",
            Self::ShieldAlert => "\u{E1FE}",

            // Execution & Status
            Self::CircleCheck => "\u{E226}",
            Self::CircleX => "\u{E088}",
            Self::AlertTriangle => "\u{E193}",
            Self::LoaderCircle => "\u{E10E}",
            Self::Play => "\u{E140}",
            Self::Pause => "\u{E132}",
            Self::Stop => "\u{E16B}",
            Self::Check => "\u{E070}",
            Self::X => "\u{E1B2}",
            Self::Clock => "\u{E08B}",
            Self::Code => "\u{E097}",
            Self::Command => "\u{E09E}",

            // Additional Interface Glyphs
            Self::Key => "\u{E102}",
            Self::Lock => "\u{E10F}",
            Self::Sliders => "\u{E29A}",
            Self::Sparkles => "\u{E417}",
            Self::Layers => "\u{E104}",
            Self::ChevronRight => "\u{E073}",
            Self::ArrowUpDown => "\u{E381}",
            Self::RefreshCw => "\u{E149}",
            Self::HelpCircle => "\u{E082}",
            Self::Info => "\u{E0FE}",
            Self::Server => "\u{E157}",
            Self::Zap => "\u{E1B4}",
            Self::User => "\u{E19F}",
            Self::Trash => "\u{E18E}",
            Self::Edit => "\u{E176}",
            Self::Eye => "\u{E0BE}",
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
            Self::User => "◈",
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

pub const ALL_ICONS: [Icon; 51] = [
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_icons_have_non_empty_glyphs_in_all_modes() {
        for icon in ALL_ICONS {
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
    fn test_lucide_mappings_against_pinned_manifest() {
        let manifest_bytes = include_bytes!("../resources/lucide_font_manifest.json");
        let manifest: serde_json::Value =
            serde_json::from_slice(manifest_bytes).expect("parse manifest");
        for icon in ALL_ICONS {
            let name = icon.lucide_icon_name();
            let entry = manifest
                .get(name)
                .unwrap_or_else(|| panic!("Icon {} missing in manifest", name));
            let expected_hex = entry["encodedCode"]
                .as_str()
                .unwrap()
                .trim_start_matches('\\');
            let expected_cp = u32::from_str_radix(expected_hex, 16).unwrap();
            let actual_char = icon.lucide_glyph().chars().next().unwrap();
            assert_eq!(
                actual_char as u32, expected_cp,
                "Icon {:?} ({}) codepoint mismatch: expected \\u{{{:04X}}}, got \\u{{{:04X}}}",
                icon, name, expected_cp, actual_char as u32
            );
        }
    }

    #[test]
    fn test_ascii_fallbacks_are_strictly_ascii() {
        for icon in ALL_ICONS {
            let s = icon.ascii_glyph();
            assert!(
                s.is_ascii(),
                "Icon {:?} ASCII fallback {:?} is not ASCII",
                icon,
                s
            );
        }
    }

    #[test]
    fn test_unicode_fallback_has_no_emoji() {
        for icon in ALL_ICONS {
            let s = icon.unicode_glyph();
            for ch in s.chars() {
                let cp = ch as u32;
                // Assert no surrogate / SMP emojis (e.g. U+1F000..U+1FFFF)
                assert!(
                    !(0x1F000..=0x1FFFF).contains(&cp),
                    "Icon {:?} contains SMP emoji character {:?} (U+{:04X})",
                    icon,
                    ch,
                    cp
                );
                // Also ensure User is not 👤 (U+1F464)
                assert_ne!(ch, '👤', "Icon {:?} still contains user emoji", icon);
            }
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
