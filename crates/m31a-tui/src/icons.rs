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
use sha2::{Digest, Sha256};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
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

// === Font Verification, Discovery, & Automatic Fallback ===

/// Pinned official Lucide static font release version.
pub const LUCIDE_FONT_VERSION: &str = "0.460.0";

/// Canonical font file name for the Lucide static TrueType font.
pub const LUCIDE_FONT_FILE_NAME: &str = "lucide.ttf";

/// Official SHA-256 checksum of `lucide.ttf` (release 0.460.0).
pub const LUCIDE_FONT_EXPECTED_SHA256: &str =
    "55e868f053500d4e4298fac5515f2d4f63a10aa7b263b022ff96c126cd9e95dc";

/// Expected byte size of official `lucide.ttf` (release 0.460.0).
pub const LUCIDE_FONT_EXPECTED_SIZE: u64 = 676_416;

/// Comprehensive verification report for a local or bundled Lucide font file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontVerificationReport {
    pub font_path: Option<PathBuf>,
    pub file_exists: bool,
    pub file_size_bytes: u64,
    pub sha256_hash: String,
    pub hash_valid: bool,
    pub valid_truetype_header: bool,
    pub missing_codepoints: Vec<u32>,
    pub covered_icons_count: usize,
    pub total_required_icons: usize,
    pub is_usable: bool,
}

/// Discovered terminal glyph capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlyphCapability {
    pub supports_lucide: bool,
    pub supports_unicode: bool,
    pub font_detected: bool,
    pub recommended_mode: IconMode,
}

/// Verifies raw font bytes against TrueType header, SHA-256, and cmap glyph coverage.
pub fn verify_font_bytes(bytes: &[u8], path: Option<PathBuf>) -> FontVerificationReport {
    let file_size_bytes = bytes.len() as u64;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash_bytes = hasher.finalize();
    let mut sha256_hash = String::with_capacity(64);
    for b in hash_bytes {
        use std::fmt::Write;
        let _ = write!(&mut sha256_hash, "{:02x}", b);
    }

    let hash_valid = sha256_hash.eq_ignore_ascii_case(LUCIDE_FONT_EXPECTED_SHA256);

    let valid_truetype_header = if bytes.len() >= 12 {
        let magic = &bytes[0..4];
        magic == [0x00, 0x01, 0x00, 0x00] || magic == b"true" || magic == b"OTTO"
    } else {
        false
    };

    // Parse cmap table to verify glyph codepoint coverage
    let mut segments = Vec::new();
    if valid_truetype_header && bytes.len() >= 12 {
        let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let mut cmap_offset: Option<usize> = None;

        for i in 0..num_tables {
            let entry_offset = 12 + i * 16;
            if entry_offset + 16 <= bytes.len() {
                let tag = &bytes[entry_offset..entry_offset + 4];
                if tag == b"cmap" {
                    let offset = u32::from_be_bytes([
                        bytes[entry_offset + 8],
                        bytes[entry_offset + 9],
                        bytes[entry_offset + 10],
                        bytes[entry_offset + 11],
                    ]) as usize;
                    cmap_offset = Some(offset);
                    break;
                }
            }
        }

        if let Some(cmap_off) = cmap_offset
            && cmap_off + 4 <= bytes.len()
        {
            let num_subtables =
                u16::from_be_bytes([bytes[cmap_off + 2], bytes[cmap_off + 3]]) as usize;
            for i in 0..num_subtables {
                let sub_rec_off = cmap_off + 4 + i * 8;
                if sub_rec_off + 8 <= bytes.len() {
                    let sub_off = u32::from_be_bytes([
                        bytes[sub_rec_off + 4],
                        bytes[sub_rec_off + 5],
                        bytes[sub_rec_off + 6],
                        bytes[sub_rec_off + 7],
                    ]) as usize;
                    let subtable_pos = cmap_off + sub_off;
                    if subtable_pos + 6 <= bytes.len() {
                        let fmt =
                            u16::from_be_bytes([bytes[subtable_pos], bytes[subtable_pos + 1]]);
                        if fmt == 4 && subtable_pos + 14 <= bytes.len() {
                            let seg_count_x2 = u16::from_be_bytes([
                                bytes[subtable_pos + 6],
                                bytes[subtable_pos + 7],
                            ]) as usize;
                            let seg_count = seg_count_x2 / 2;
                            let end_code_start = subtable_pos + 14;
                            let start_code_start = end_code_start + seg_count * 2 + 2;

                            if start_code_start + seg_count * 2 <= bytes.len() {
                                for s in 0..seg_count {
                                    let end_c = u16::from_be_bytes([
                                        bytes[end_code_start + s * 2],
                                        bytes[end_code_start + s * 2 + 1],
                                    ]) as u32;
                                    let start_c = u16::from_be_bytes([
                                        bytes[start_code_start + s * 2],
                                        bytes[start_code_start + s * 2 + 1],
                                    ]) as u32;
                                    segments.push((start_c, end_c));
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let total_required_icons = ALL_ICONS.len();
    let mut missing_codepoints = Vec::new();
    let mut covered_icons_count = 0;

    for icon in ALL_ICONS {
        if let Some(first_char) = icon.lucide_glyph().chars().next() {
            let cp = first_char as u32;
            let covered = if !segments.is_empty() {
                segments.iter().any(|&(s, e)| s <= cp && cp <= e)
            } else {
                hash_valid
            };

            if covered {
                covered_icons_count += 1;
            } else {
                missing_codepoints.push(cp);
            }
        }
    }

    let is_usable = valid_truetype_header
        && missing_codepoints.is_empty()
        && (hash_valid || file_size_bytes >= 500_000);

    FontVerificationReport {
        font_path: path,
        file_exists: true,
        file_size_bytes,
        sha256_hash,
        hash_valid,
        valid_truetype_header,
        missing_codepoints,
        covered_icons_count,
        total_required_icons,
        is_usable,
    }
}

/// Verifies a font file on the local filesystem.
pub fn verify_font_file(path: &Path) -> FontVerificationReport {
    if !path.exists() || !path.is_file() {
        return FontVerificationReport {
            font_path: Some(path.to_path_buf()),
            file_exists: false,
            file_size_bytes: 0,
            sha256_hash: String::new(),
            hash_valid: false,
            valid_truetype_header: false,
            missing_codepoints: ALL_ICONS
                .iter()
                .filter_map(|i| i.lucide_glyph().chars().next().map(|c| c as u32))
                .collect(),
            covered_icons_count: 0,
            total_required_icons: ALL_ICONS.len(),
            is_usable: false,
        };
    }

    match fs::read(path) {
        Ok(bytes) => verify_font_bytes(&bytes, Some(path.to_path_buf())),
        Err(_) => FontVerificationReport {
            font_path: Some(path.to_path_buf()),
            file_exists: true,
            file_size_bytes: 0,
            sha256_hash: String::new(),
            hash_valid: false,
            valid_truetype_header: false,
            missing_codepoints: ALL_ICONS
                .iter()
                .filter_map(|i| i.lucide_glyph().chars().next().map(|c| c as u32))
                .collect(),
            covered_icons_count: 0,
            total_required_icons: ALL_ICONS.len(),
            is_usable: false,
        },
    }
}

/// Returns standard operating system and distribution font search paths for Lucide.
pub fn get_lucide_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(p) = env::var("M31A_LUCIDE_FONT_PATH") {
        paths.push(PathBuf::from(p));
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local/share/fonts/lucide.ttf"));
        paths.push(home.join(".fonts/lucide.ttf"));
        paths.push(home.join(".local/share/fonts/truetype/lucide/lucide.ttf"));
        paths.push(home.join("Library/Fonts/lucide.ttf"));
    }

    paths.push(PathBuf::from("/usr/local/share/fonts/lucide.ttf"));
    paths.push(PathBuf::from("/usr/share/fonts/lucide.ttf"));
    paths.push(PathBuf::from("/usr/share/fonts/truetype/lucide/lucide.ttf"));
    paths.push(PathBuf::from("/usr/share/fonts/TTF/lucide.ttf"));
    paths.push(PathBuf::from("/Library/Fonts/lucide.ttf"));
    paths.push(PathBuf::from(
        "/System/Library/Fonts/Supplemental/lucide.ttf",
    ));

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(local_app_data).join("Microsoft/Windows/Fonts/lucide.ttf"));
    }
    if let Ok(windir) = env::var("WINDIR") {
        paths.push(PathBuf::from(windir).join("Fonts/lucide.ttf"));
    }

    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        paths.push(parent.join("resources/fonts/lucide.ttf"));
        paths.push(parent.join("resources/lucide.ttf"));
        paths.push(parent.join("fonts/lucide.ttf"));
        if let Some(grandparent) = parent.parent() {
            paths.push(grandparent.join("resources/fonts/lucide.ttf"));
            paths.push(grandparent.join("assets/fonts/lucide.ttf"));
        }
    }

    paths.push(PathBuf::from("assets/fonts/lucide.ttf"));
    paths.push(PathBuf::from("crates/m31a-tui/resources/lucide.ttf"));

    paths
}

/// Locates a valid, verified Lucide font file installed on the system or distributed alongside M31A.
pub fn find_installed_lucide_font() -> Option<PathBuf> {
    for p in get_lucide_search_paths() {
        if p.is_file() {
            let report = verify_font_file(&p);
            if report.is_usable {
                return Some(p);
            }
        }
    }
    None
}

/// Determines whether the current terminal and host environment can safely render Lucide glyphs.
pub fn detect_glyph_capability() -> GlyphCapability {
    if let Ok(val) = env::var("M31A_ICONS").or_else(|_| env::var("M31A_ICON_MODE")) {
        match val.trim().to_lowercase().as_str() {
            "ascii" => {
                return GlyphCapability {
                    supports_lucide: false,
                    supports_unicode: false,
                    font_detected: false,
                    recommended_mode: IconMode::Ascii,
                };
            }
            "unicode" => {
                return GlyphCapability {
                    supports_lucide: false,
                    supports_unicode: true,
                    font_detected: false,
                    recommended_mode: IconMode::Unicode,
                };
            }
            "lucide" => {
                let font_found = find_installed_lucide_font().is_some();
                return GlyphCapability {
                    supports_lucide: true,
                    supports_unicode: true,
                    font_detected: font_found,
                    recommended_mode: IconMode::Lucide,
                };
            }
            _ => {}
        }
    }

    if env::var("M31A_ASCII").is_ok() {
        return GlyphCapability {
            supports_lucide: false,
            supports_unicode: false,
            font_detected: false,
            recommended_mode: IconMode::Ascii,
        };
    }

    let term = env::var("TERM").unwrap_or_default().to_lowercase();
    if term == "dumb" || term == "vt100" || term == "vt220" {
        return GlyphCapability {
            supports_lucide: false,
            supports_unicode: false,
            font_detected: false,
            recommended_mode: IconMode::Ascii,
        };
    }

    let lang = env::var("LC_ALL")
        .or_else(|_| env::var("LC_CTYPE"))
        .or_else(|_| env::var("LANG"))
        .unwrap_or_default()
        .to_lowercase();
    let supports_unicode = lang.contains("utf-8") || lang.contains("utf8") || lang.is_empty();

    if !supports_unicode {
        return GlyphCapability {
            supports_lucide: false,
            supports_unicode: false,
            font_detected: false,
            recommended_mode: IconMode::Ascii,
        };
    }

    let user_confirmed_nerd_font = env::var("M31A_NERD_FONTS").is_ok()
        || env::var("M31A_LUCIDE_FONT").is_ok()
        || env::var("NERD_FONT").is_ok();

    let font_detected = find_installed_lucide_font().is_some();
    let supports_lucide = user_confirmed_nerd_font || font_detected;

    let recommended_mode = if supports_lucide {
        IconMode::Lucide
    } else {
        IconMode::Unicode
    };

    GlyphCapability {
        supports_lucide,
        supports_unicode: true,
        font_detected,
        recommended_mode,
    }
}

/// Resolves a requested [`IconMode`] to a guaranteed-safe mode considering terminal and font support.
pub fn resolve_safe_icon_mode(requested: IconMode) -> IconMode {
    let cap = detect_glyph_capability();
    match requested {
        IconMode::Lucide => {
            if cap.supports_lucide {
                IconMode::Lucide
            } else if cap.supports_unicode {
                IconMode::Unicode
            } else {
                IconMode::Ascii
            }
        }
        IconMode::Unicode => {
            if cap.supports_unicode {
                IconMode::Unicode
            } else {
                IconMode::Ascii
            }
        }
        IconMode::Ascii => IconMode::Ascii,
    }
}

/// Helper to install a bundled Lucide font into the current user's local font library.
pub fn install_lucide_font_to_user_fonts(font_bytes: &[u8]) -> std::io::Result<PathBuf> {
    let report = verify_font_bytes(font_bytes, None);
    if !report.is_usable {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Font data failed TrueType / Lucide verification",
        ));
    }

    let target_dir = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home dir"))?
            .join("Library/Fonts")
    } else if cfg!(target_os = "windows") {
        if let Ok(app_data) = env::var("LOCALAPPDATA") {
            PathBuf::from(app_data).join("Microsoft/Windows/Fonts")
        } else {
            dirs::home_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home dir"))?
                .join(".fonts")
        }
    } else {
        if let Ok(xdg) = env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg).join("fonts")
        } else {
            dirs::home_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home dir"))?
                .join(".local/share/fonts")
        }
    };

    fs::create_dir_all(&target_dir)?;
    let dest_file = target_dir.join(LUCIDE_FONT_FILE_NAME);
    fs::write(&dest_file, font_bytes)?;

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("fc-cache")
            .arg("-f")
            .arg(&target_dir)
            .output();
    }

    Ok(dest_file)
}

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
        crate::theme::set_icon_mode(None);
        let mut dark = crate::theme::Theme::dark();
        assert_eq!(
            Icon::Command.render(&dark),
            Icon::Command.glyph(dark.icon_mode)
        );

        // When use_icons is disabled, ASCII fallback is strictly used
        dark.typography.use_icons = false;
        assert_eq!(Icon::Command.render(&dark), Icon::Command.ascii_glyph());
        crate::theme::set_icon_mode(None);
    }

    #[test]
    fn test_verify_bundled_lucide_font() {
        let font_path = Path::new("resources/lucide.ttf");
        if font_path.exists() {
            let report = verify_font_file(font_path);
            assert!(report.file_exists);
            assert!(report.valid_truetype_header);
            assert_eq!(report.sha256_hash, LUCIDE_FONT_EXPECTED_SHA256);
            assert!(report.hash_valid);
            assert_eq!(report.covered_icons_count, ALL_ICONS.len());
            assert!(report.missing_codepoints.is_empty());
            assert!(report.is_usable);
        }
    }

    #[test]
    fn test_font_verification_invalid_bytes() {
        let bad_bytes = b"not a true type font";
        let report = verify_font_bytes(bad_bytes, None);
        assert!(!report.valid_truetype_header);
        assert!(!report.hash_valid);
        assert!(!report.is_usable);
        assert!(!report.missing_codepoints.is_empty());
    }

    #[test]
    fn test_glyph_capability_detection_and_fallback() {
        // Test ASCII override
        unsafe { env::set_var("M31A_ICONS", "ascii") };
        let cap = detect_glyph_capability();
        assert_eq!(cap.recommended_mode, IconMode::Ascii);
        assert_eq!(resolve_safe_icon_mode(IconMode::Lucide), IconMode::Ascii);
        assert_eq!(resolve_safe_icon_mode(IconMode::Unicode), IconMode::Ascii);
        assert_eq!(resolve_safe_icon_mode(IconMode::Ascii), IconMode::Ascii);
        unsafe { env::remove_var("M31A_ICONS") };

        // Test Unicode override
        unsafe { env::set_var("M31A_ICONS", "unicode") };
        let cap = detect_glyph_capability();
        assert_eq!(cap.recommended_mode, IconMode::Unicode);
        assert_eq!(resolve_safe_icon_mode(IconMode::Lucide), IconMode::Unicode);
        assert_eq!(resolve_safe_icon_mode(IconMode::Unicode), IconMode::Unicode);
        assert_eq!(resolve_safe_icon_mode(IconMode::Ascii), IconMode::Ascii);
        unsafe { env::remove_var("M31A_ICONS") };

        // Test Dumb terminal
        unsafe { env::set_var("TERM", "dumb") };
        let cap = detect_glyph_capability();
        assert_eq!(cap.recommended_mode, IconMode::Ascii);
        assert_eq!(resolve_safe_icon_mode(IconMode::Lucide), IconMode::Ascii);
        unsafe { env::remove_var("TERM") };
    }

    #[test]
    fn test_font_install_helper_validation() {
        let invalid_bytes = b"corrupt font bytes";
        let res = install_lucide_font_to_user_fonts(invalid_bytes);
        assert!(res.is_err());
    }
}
