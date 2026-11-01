//! Terminal emulator probe and capability detection.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalKind {
    Ghostty,
    Kitty,
    ITerm2,
    Alacritty,
    WezTerm,
    VSCode,
    WindowsTerminal,
    WarpTerminal,
    Tmux,
    AppleTerminal,
    Foot,
    Dumb,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorLevel {
    Truecolor,
    Ansi256,
    Ansi16,
    NoColor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub kind: TerminalKind,
    pub name: String,
    pub color_level: ColorLevel,
    pub supports_hyperlinks: bool,
    pub supports_bracketed_paste: bool,
    pub supports_kitty_keyboard: bool,
    pub supports_synchronized_updates: bool,
    pub supports_graphics_protocol: bool,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();

        let kind = if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() || term_program == "ghostty" {
            TerminalKind::Ghostty
        } else if std::env::var("KITTY_WINDOW_ID").is_ok() || term.contains("kitty") {
            TerminalKind::Kitty
        } else if term_program == "iTerm.app" {
            TerminalKind::ITerm2
        } else if term_program == "WezTerm" || std::env::var("WEZTERM_PANE").is_ok() {
            TerminalKind::WezTerm
        } else if term.contains("alacritty") || term_program == "Alacritty" {
            TerminalKind::Alacritty
        } else if term_program == "vscode" {
            TerminalKind::VSCode
        } else if term_program == "WarpTerminal"
            || std::env::var("WARP_IS_LOCAL_SHELL_SESSION").is_ok()
        {
            TerminalKind::WarpTerminal
        } else if std::env::var("WT_SESSION").is_ok() {
            TerminalKind::WindowsTerminal
        } else if std::env::var("TMUX").is_ok() {
            TerminalKind::Tmux
        } else if term_program == "Apple_Terminal" {
            TerminalKind::AppleTerminal
        } else if term == "foot" || term == "foot-extra" {
            TerminalKind::Foot
        } else if term == "dumb" {
            TerminalKind::Dumb
        } else {
            TerminalKind::Unknown
        };

        let color_level = if std::env::var("NO_COLOR").is_ok() || term == "dumb" {
            ColorLevel::NoColor
        } else if colorterm == "truecolor"
            || colorterm == "24bit"
            || matches!(
                kind,
                TerminalKind::Ghostty
                    | TerminalKind::Kitty
                    | TerminalKind::ITerm2
                    | TerminalKind::WezTerm
                    | TerminalKind::Alacritty
                    | TerminalKind::VSCode
                    | TerminalKind::WindowsTerminal
                    | TerminalKind::WarpTerminal
            )
        {
            ColorLevel::Truecolor
        } else if term.contains("256color") {
            ColorLevel::Ansi256
        } else {
            ColorLevel::Ansi16
        };

        let supports_hyperlinks = matches!(
            kind,
            TerminalKind::Ghostty
                | TerminalKind::Kitty
                | TerminalKind::ITerm2
                | TerminalKind::WezTerm
                | TerminalKind::Alacritty
                | TerminalKind::VSCode
                | TerminalKind::WindowsTerminal
                | TerminalKind::WarpTerminal
                | TerminalKind::Foot
        );

        let supports_bracketed_paste = kind != TerminalKind::Dumb;

        let supports_kitty_keyboard = matches!(
            kind,
            TerminalKind::Kitty
                | TerminalKind::Ghostty
                | TerminalKind::WezTerm
                | TerminalKind::Foot
        );

        let supports_synchronized_updates = matches!(
            kind,
            TerminalKind::Ghostty
                | TerminalKind::Kitty
                | TerminalKind::WezTerm
                | TerminalKind::Alacritty
                | TerminalKind::Foot
        );

        let supports_graphics_protocol = matches!(
            kind,
            TerminalKind::Kitty
                | TerminalKind::Ghostty
                | TerminalKind::WezTerm
                | TerminalKind::ITerm2
        );

        let name = match kind {
            TerminalKind::Ghostty => "Ghostty".to_string(),
            TerminalKind::Kitty => "Kitty".to_string(),
            TerminalKind::ITerm2 => "iTerm2".to_string(),
            TerminalKind::Alacritty => "Alacritty".to_string(),
            TerminalKind::WezTerm => "WezTerm".to_string(),
            TerminalKind::VSCode => "VS Code Integrated Terminal".to_string(),
            TerminalKind::WindowsTerminal => "Windows Terminal".to_string(),
            TerminalKind::WarpTerminal => "Warp".to_string(),
            TerminalKind::Tmux => "tmux".to_string(),
            TerminalKind::AppleTerminal => "Terminal.app".to_string(),
            TerminalKind::Foot => "Foot".to_string(),
            TerminalKind::Dumb => "Dumb Terminal".to_string(),
            TerminalKind::Unknown => {
                if !term.is_empty() {
                    term
                } else {
                    "Unknown Terminal".to_string()
                }
            }
        };

        Self {
            kind,
            name,
            color_level,
            supports_hyperlinks,
            supports_bracketed_paste,
            supports_kitty_keyboard,
            supports_synchronized_updates,
            supports_graphics_protocol,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_smoke() {
        let caps = TerminalCapabilities::detect();
        assert!(!caps.name.is_empty());
    }
}
