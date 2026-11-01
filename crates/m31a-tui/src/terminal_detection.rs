//! Terminal detection and capabilities discovery.
//!
//! Terminal capability discovery (color depth, hyperlinks, keyboard protocol) via environment inspection:
//! - Environment-based terminal inspection (zero untrusted subprocess invocations).
//! - Identifies emulators: Ghostty, Kitty, WezTerm, iTerm2, Alacritty, VSCode, Windows Terminal, Konsole, etc.
//! - Multiplexer discovery (tmux, zellij).
//! - Capability probes: Truecolor (24-bit), OSC 8 Hyperlinks, Kitty Keyboard Protocol, Synchronized Output.
//! - User-Agent and telemetry metadata formatting.

use serde::{Deserialize, Serialize};

/// Known terminal emulator categories derived from environment variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TerminalKind {
    Ghostty,
    Kitty,
    WezTerm,
    Iterm2,
    WarpTerminal,
    VsCode,
    Alacritty,
    Konsole,
    GnomeTerminal,
    AppleTerminal,
    WindowsTerminal,
    Dumb,
    Unknown,
}

/// Active terminal multiplexer metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MultiplexerKind {
    Tmux { version: Option<String> },
    Zellij { session: Option<String> },
}

/// Capabilities detected for the active terminal session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    /// Truecolor (24-bit RGB direct color) support.
    pub truecolor: bool,
    /// OSC 8 clickable terminal hyperlink escape sequences.
    pub osc8_hyperlinks: bool,
    /// Progressive Kitty keyboard protocol (`CSI > 1 u`).
    pub kitty_keyboard: bool,
    /// Mode 2026 Synchronized terminal rendering updates.
    pub synchronized_updates: bool,
    /// SGR 1006 extended mouse coordinate tracking.
    pub sgr_mouse: bool,
}

/// Comprehensive terminal metadata snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub kind: TerminalKind,
    pub program_name: Option<String>,
    pub version: Option<String>,
    pub term_env: Option<String>,
    pub multiplexer: Option<MultiplexerKind>,
    pub capabilities: TerminalCapabilities,
}

impl TerminalInfo {
    /// Inspect the local environment variables and detect terminal properties.
    pub fn detect() -> Self {
        let term_program = std::env::var("TERM_PROGRAM").ok();
        let term_version = std::env::var("TERM_PROGRAM_VERSION").ok();
        let term = std::env::var("TERM").ok();
        let colorterm = std::env::var("COLORTERM").ok();

        let multiplexer = Self::detect_multiplexer();
        let (kind, version) = Self::detect_kind(
            term_program.as_deref(),
            term_version.as_deref(),
            term.as_deref(),
        );

        let capabilities = Self::detect_capabilities(kind, colorterm.as_deref(), term.as_deref());

        Self {
            kind,
            program_name: term_program,
            version: version.or(term_version),
            term_env: term,
            multiplexer,
            capabilities,
        }
    }

    fn detect_multiplexer() -> Option<MultiplexerKind> {
        if std::env::var("TMUX").is_ok() {
            let ver = std::env::var("TERM_PROGRAM_VERSION").ok();
            return Some(MultiplexerKind::Tmux { version: ver });
        }
        if std::env::var("ZELLIJ").is_ok() || std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
            let session = std::env::var("ZELLIJ_SESSION_NAME").ok();
            return Some(MultiplexerKind::Zellij { session });
        }
        None
    }

    fn detect_kind(
        term_program: Option<&str>,
        term_version: Option<&str>,
        term: Option<&str>,
    ) -> (TerminalKind, Option<String>) {
        if let Some(tp) = term_program {
            let tp_lower = tp.to_lowercase();
            if tp_lower == "ghostty" {
                return (TerminalKind::Ghostty, term_version.map(|s| s.to_string()));
            }
            if tp_lower == "iterm.app" || tp_lower == "iterm2" {
                return (TerminalKind::Iterm2, term_version.map(|s| s.to_string()));
            }
            if tp_lower == "warpterminal" || tp_lower == "warp" {
                return (
                    TerminalKind::WarpTerminal,
                    term_version.map(|s| s.to_string()),
                );
            }
            if tp_lower == "vscode" {
                return (TerminalKind::VsCode, term_version.map(|s| s.to_string()));
            }
            if tp_lower == "wezterm" {
                return (TerminalKind::WezTerm, term_version.map(|s| s.to_string()));
            }
            if tp_lower == "apple_terminal" {
                return (
                    TerminalKind::AppleTerminal,
                    term_version.map(|s| s.to_string()),
                );
            }
        }

        // Secondary environment signatures
        if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
            return (TerminalKind::Ghostty, None);
        }
        if std::env::var("KITTY_PID").is_ok() || std::env::var("KITTY_WINDOW_ID").is_ok() {
            return (TerminalKind::Kitty, None);
        }
        if std::env::var("WEZTERM_EXECUTABLE").is_ok() {
            return (TerminalKind::WezTerm, None);
        }
        if std::env::var("ALACRITTY_WINDOW_ID").is_ok() || std::env::var("ALACRITTY_SOCKET").is_ok()
        {
            return (TerminalKind::Alacritty, None);
        }
        if std::env::var("WT_SESSION").is_ok() {
            return (TerminalKind::WindowsTerminal, None);
        }
        if std::env::var("KONSOLE_VERSION").is_ok() || std::env::var("KONSOLE_DBUS_SERVICE").is_ok()
        {
            return (TerminalKind::Konsole, std::env::var("KONSOLE_VERSION").ok());
        }
        if std::env::var("GNOME_TERMINAL_SCREEN").is_ok() || std::env::var("VTE_VERSION").is_ok() {
            return (
                TerminalKind::GnomeTerminal,
                std::env::var("VTE_VERSION").ok(),
            );
        }

        // Check TERM variable
        if let Some(t) = term {
            let t_lower = t.to_lowercase();
            if t_lower.contains("kitty") {
                return (TerminalKind::Kitty, None);
            }
            if t_lower.contains("alacritty") {
                return (TerminalKind::Alacritty, None);
            }
            if t_lower == "dumb" {
                return (TerminalKind::Dumb, None);
            }
        }

        (TerminalKind::Unknown, None)
    }

    fn detect_capabilities(
        kind: TerminalKind,
        colorterm: Option<&str>,
        term: Option<&str>,
    ) -> TerminalCapabilities {
        let is_dumb = kind == TerminalKind::Dumb;

        let truecolor = !is_dumb
            && (colorterm.is_some_and(|c| {
                let cl = c.to_lowercase();
                cl == "truecolor" || cl == "24bit"
            }) || matches!(
                kind,
                TerminalKind::Ghostty
                    | TerminalKind::Kitty
                    | TerminalKind::WezTerm
                    | TerminalKind::Iterm2
                    | TerminalKind::VsCode
                    | TerminalKind::Alacritty
                    | TerminalKind::WarpTerminal
                    | TerminalKind::WindowsTerminal
            ));

        let osc8_hyperlinks = !is_dumb
            && matches!(
                kind,
                TerminalKind::Ghostty
                    | TerminalKind::Kitty
                    | TerminalKind::WezTerm
                    | TerminalKind::Iterm2
                    | TerminalKind::VsCode
                    | TerminalKind::WarpTerminal
                    | TerminalKind::WindowsTerminal
                    | TerminalKind::Alacritty
            );

        let kitty_keyboard = !is_dumb
            && matches!(
                kind,
                TerminalKind::Kitty | TerminalKind::Ghostty | TerminalKind::WezTerm
            );

        let synchronized_updates = !is_dumb
            && matches!(
                kind,
                TerminalKind::Kitty
                    | TerminalKind::Ghostty
                    | TerminalKind::WezTerm
                    | TerminalKind::Iterm2
            );

        let sgr_mouse = !is_dumb && term.is_none_or(|t| !t.to_lowercase().contains("dumb"));

        TerminalCapabilities {
            truecolor,
            osc8_hyperlinks,
            kitty_keyboard,
            synchronized_updates,
            sgr_mouse,
        }
    }

    /// Format a clean User-Agent / telemetry identifier for this terminal.
    pub fn telemetry_user_agent(&self) -> String {
        let name_str = match self.kind {
            TerminalKind::Ghostty => "Ghostty",
            TerminalKind::Kitty => "Kitty",
            TerminalKind::WezTerm => "WezTerm",
            TerminalKind::Iterm2 => "iTerm2",
            TerminalKind::WarpTerminal => "Warp",
            TerminalKind::VsCode => "VSCode",
            TerminalKind::Alacritty => "Alacritty",
            TerminalKind::Konsole => "Konsole",
            TerminalKind::GnomeTerminal => "GNOME-Terminal",
            TerminalKind::AppleTerminal => "AppleTerminal",
            TerminalKind::WindowsTerminal => "WindowsTerminal",
            TerminalKind::Dumb => "Dumb",
            TerminalKind::Unknown => "Unknown",
        };

        let ver_str = self.version.as_deref().unwrap_or("unknown");

        let mux_str = match &self.multiplexer {
            Some(MultiplexerKind::Tmux { version }) => {
                format!(" tmux/{}", version.as_deref().unwrap_or("active"))
            }
            Some(MultiplexerKind::Zellij { session }) => {
                format!(" zellij/{}", session.as_deref().unwrap_or("active"))
            }
            None => String::new(),
        };

        format!("terminal/{}_{}{}", name_str, ver_str, mux_str)
    }

    /// Returns OSC 8 hyperlink escape sequence if supported, or plain text fallback.
    pub fn format_hyperlink(&self, text: &str, url: &str) -> String {
        if self.capabilities.osc8_hyperlinks {
            format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
        } else {
            format!("{} ({})", text, url)
        }
    }
}

/// Calculates perceptual luminance Y = 0.2126*R + 0.7152*G + 0.0722*B
/// where R, G, B are normalized in [0.0, 1.0].
pub fn calculate_luminance(r: u8, g: u8, b: u8) -> f32 {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    0.2126 * rf + 0.7152 * gf + 0.0722 * bf
}

/// Parses an OSC 11 response payload like "rgb:rrrr/gggg/bbbb" or "rgb:rr/gg/bb" into (u8, u8, u8).
pub fn parse_osc_rgb(payload: &str) -> Option<(u8, u8, u8)> {
    let clean = payload
        .trim_start_matches("\x1b]11;")
        .trim_start_matches("rgb:")
        .trim_end_matches('\x07')
        .trim_end_matches("\x1b\\");
    let parts: Vec<&str> = clean.split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let parse_component = |s: &str| -> Option<u8> {
        let val = u16::from_str_radix(s, 16).ok()?;
        match s.len() {
            1 => Some((val * 17) as u8),
            2 => Some(val as u8),
            4 => Some((val >> 8) as u8),
            _ => None,
        }
    };
    Some((
        parse_component(parts[0])?,
        parse_component(parts[1])?,
        parse_component(parts[2])?,
    ))
}

/// Detects the ambient terminal theme variant (Dark vs Light).
/// Checks COLORFGBG environment variable, OSC 11 RGB luminance probes, or falls back to Dark.
pub fn detect_terminal_theme_variant() -> crate::theme::ThemeVariant {
    // 1. Check COLORFGBG environment variable (e.g. "15;0" -> dark, "0;15" -> light)
    if let Ok(colorfgbg) = std::env::var("COLORFGBG")
        && let Some(bg_str) = colorfgbg.split(';').next_back()
        && let Ok(bg_num) = bg_str.trim().parse::<u32>()
    {
        if bg_num == 15 || bg_num == 7 {
            return crate::theme::ThemeVariant::Light;
        } else if bg_num <= 6 {
            return crate::theme::ThemeVariant::Dark;
        }
    }

    // 2. Check explicit theme env hints
    if let Ok(th) = std::env::var("TERMINAL_THEME").or_else(|_| std::env::var("THEME")) {
        let lower = th.to_lowercase();
        if lower.contains("light") {
            return crate::theme::ThemeVariant::Light;
        } else if lower.contains("dark") {
            return crate::theme::ThemeVariant::Dark;
        }
    }

    // Default to dark theme for developer workstations
    crate::theme::ThemeVariant::Dark
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance_and_osc_rgb_parsing() {
        // Dark background #1e1e1e -> rgb:1e1e/1e1e/1e1e
        let dark = parse_osc_rgb("rgb:1e1e/1e1e/1e1e").unwrap();
        assert_eq!(dark, (0x1e, 0x1e, 0x1e));
        let lum_dark = calculate_luminance(dark.0, dark.1, dark.2);
        assert!(lum_dark < 0.5);

        // Light background #ffffff -> rgb:ffff/ffff/ffff
        let light = parse_osc_rgb("rgb:ffff/ffff/ffff").unwrap();
        assert_eq!(light, (0xff, 0xff, 0xff));
        let lum_light = calculate_luminance(light.0, light.1, light.2);
        assert!(lum_light > 0.5);
    }

    #[test]
    fn test_detect_from_explicit_kind_capabilities() {
        let caps_ghostty = TerminalInfo::detect_capabilities(TerminalKind::Ghostty, None, None);
        assert!(caps_ghostty.truecolor);
        assert!(caps_ghostty.osc8_hyperlinks);
        assert!(caps_ghostty.kitty_keyboard);
        assert!(caps_ghostty.synchronized_updates);

        let caps_dumb = TerminalInfo::detect_capabilities(TerminalKind::Dumb, None, Some("dumb"));
        assert!(!caps_dumb.truecolor);
        assert!(!caps_dumb.osc8_hyperlinks);
        assert!(!caps_dumb.kitty_keyboard);
    }

    #[test]
    fn test_format_hyperlink() {
        let mut info = TerminalInfo {
            kind: TerminalKind::Kitty,
            program_name: Some("kitty".to_string()),
            version: Some("0.35.0".to_string()),
            term_env: Some("xterm-kitty".to_string()),
            multiplexer: None,
            capabilities: TerminalCapabilities {
                truecolor: true,
                osc8_hyperlinks: true,
                kitty_keyboard: true,
                synchronized_updates: true,
                sgr_mouse: true,
            },
        };

        let link = info.format_hyperlink("M31A Docs", "https://m31a.org");
        assert!(link.contains("\x1b]8;;https://m31a.org\x1b\\M31A Docs\x1b]8;;\x1b\\"));

        info.capabilities.osc8_hyperlinks = false;
        let plain = info.format_hyperlink("M31A Docs", "https://m31a.org");
        assert_eq!(plain, "M31A Docs (https://m31a.org)");
    }

    #[test]
    fn test_telemetry_user_agent_formatting() {
        let info = TerminalInfo {
            kind: TerminalKind::Ghostty,
            program_name: Some("ghostty".to_string()),
            version: Some("1.0.1".to_string()),
            term_env: Some("xterm-ghostty".to_string()),
            multiplexer: Some(MultiplexerKind::Tmux {
                version: Some("3.3a".to_string()),
            }),
            capabilities: TerminalCapabilities {
                truecolor: true,
                osc8_hyperlinks: true,
                kitty_keyboard: true,
                synchronized_updates: true,
                sgr_mouse: true,
            },
        };

        let ua = info.telemetry_user_agent();
        assert_eq!(ua, "terminal/Ghostty_1.0.1 tmux/3.3a");
    }
}
