//! Terminal emulator probe, color support verification, and capability queries.
//!
//! Probes terminal features (24-bit truecolor, OSC 8 hyperlinks, Unicode width) to optimize rendering.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
    Ansi16,
    NoColor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalName {
    AppleTerminal,
    Ghostty,
    Iterm2,
    WarpTerminal,
    VsCode,
    WezTerm,
    Kitty,
    Alacritty,
    Konsole,
    GnomeTerminal,
    Vte,
    WindowsTerminal,
    Dumb,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Multiplexer {
    Tmux { version: Option<String> },
    Zellij { version: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalInfo {
    pub name: TerminalName,
    pub term_program: Option<String>,
    pub version: Option<String>,
    pub term: Option<String>,
    pub multiplexer: Option<Multiplexer>,
    pub is_tty: bool,
    pub color_depth: ColorDepth,
}

impl TerminalInfo {
    pub fn detect() -> Self {
        detect_terminal_from_env(&SystemEnvironment)
    }

    pub fn display_summary(&self) -> String {
        format!(
            "Terminal: {:?}\nProgram: {}\nVersion: {}\nTERM: {}\nMultiplexer: {:?}\nColor Depth: {:?}\nTTY: {}\nUser-Agent: {}\n",
            self.name,
            self.term_program.as_deref().unwrap_or("none"),
            self.version.as_deref().unwrap_or("none"),
            self.term.as_deref().unwrap_or("none"),
            self.multiplexer,
            self.color_depth,
            self.is_tty,
            self.user_agent_token()
        )
    }

    pub fn new(
        name: TerminalName,
        term_program: Option<String>,
        version: Option<String>,
        term: Option<String>,
        multiplexer: Option<Multiplexer>,
        is_tty: bool,
        color_depth: ColorDepth,
    ) -> Self {
        Self {
            name,
            term_program,
            version,
            term,
            multiplexer,
            is_tty,
            color_depth,
        }
    }

    pub fn is_zellij(&self) -> bool {
        matches!(self.multiplexer, Some(Multiplexer::Zellij { .. }))
    }

    pub fn is_tmux(&self) -> bool {
        matches!(self.multiplexer, Some(Multiplexer::Tmux { .. }))
    }

    /// Formats the terminal info as a User-Agent token.
    pub fn user_agent_token(&self) -> String {
        let raw = if let Some(program) = self.term_program.as_ref() {
            match self.version.as_ref().filter(|v| !v.is_empty()) {
                Some(version) => format!("{program}/{version}"),
                None => program.clone(),
            }
        } else if let Some(term) = self.term.as_ref().filter(|value| !value.is_empty()) {
            term.clone()
        } else {
            match self.name {
                TerminalName::AppleTerminal => format_version("Apple_Terminal", &self.version),
                TerminalName::Ghostty => format_version("Ghostty", &self.version),
                TerminalName::Iterm2 => format_version("iTerm.app", &self.version),
                TerminalName::WarpTerminal => format_version("WarpTerminal", &self.version),
                TerminalName::VsCode => format_version("vscode", &self.version),
                TerminalName::WezTerm => format_version("WezTerm", &self.version),
                TerminalName::Kitty => "kitty".to_string(),
                TerminalName::Alacritty => "Alacritty".to_string(),
                TerminalName::Konsole => format_version("Konsole", &self.version),
                TerminalName::GnomeTerminal => "gnome-terminal".to_string(),
                TerminalName::Vte => format_version("VTE", &self.version),
                TerminalName::WindowsTerminal => "WindowsTerminal".to_string(),
                TerminalName::Dumb => "dumb".to_string(),
                TerminalName::Unknown => "unknown".to_string(),
            }
        };

        sanitize_header_value(&raw)
    }
}

fn format_version(name: &str, version: &Option<String>) -> String {
    match version.as_deref().filter(|v| !v.is_empty()) {
        Some(v) => format!("{name}/{v}"),
        None => name.to_string(),
    }
}

fn sanitize_header_value(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_graphic() { c } else { '_' })
        .collect()
}

pub trait Environment {
    fn var(&self, key: &str) -> Option<String>;
}

pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// Detects terminal metadata from environment variables.
pub fn detect_terminal_from_env<E: Environment>(env: &E) -> TerminalInfo {
    let multiplexer = detect_multiplexer(env);
    let color_depth = detect_color_depth(env);
    let is_tty = env.var("TERM").as_deref() != Some("dumb");

    // 1. Check TERM_PROGRAM
    if let Some(program) = env.var("TERM_PROGRAM").filter(|s| !s.is_empty()) {
        let version = env.var("TERM_PROGRAM_VERSION").filter(|s| !s.is_empty());
        let name = match program.as_str() {
            "Apple_Terminal" => TerminalName::AppleTerminal,
            "ghostty" => TerminalName::Ghostty,
            "iTerm.app" => TerminalName::Iterm2,
            "WarpTerminal" => TerminalName::WarpTerminal,
            "vscode" => TerminalName::VsCode,
            "WezTerm" => TerminalName::WezTerm,
            _ => TerminalName::Unknown,
        };
        return TerminalInfo::new(
            name,
            Some(program),
            version,
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    // 2. Specific terminal emulator environment signatures
    if env.var("KITTY_PID").is_some() || env.var("KITTY_WINDOW_ID").is_some() {
        return TerminalInfo::new(
            TerminalName::Kitty,
            None,
            None,
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    if env.var("ALACRITTY_LOG").is_some() || env.var("ALACRITTY_WINDOW_ID").is_some() {
        return TerminalInfo::new(
            TerminalName::Alacritty,
            None,
            None,
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    if let Some(v) = env.var("KONSOLE_VERSION") {
        return TerminalInfo::new(
            TerminalName::Konsole,
            None,
            Some(v),
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    if let Some(v) = env.var("VTE_VERSION") {
        return TerminalInfo::new(
            TerminalName::Vte,
            None,
            Some(v),
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    if env.var("WT_SESSION").is_some() {
        return TerminalInfo::new(
            TerminalName::WindowsTerminal,
            None,
            None,
            None,
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    // 3. Fallback to TERM
    if let Some(term) = env.var("TERM").filter(|s| !s.is_empty()) {
        let name = match term.as_str() {
            "dumb" => TerminalName::Dumb,
            "xterm-ghostty" => TerminalName::Ghostty,
            "wezterm" | "wezterm-mux" => TerminalName::WezTerm,
            _ => TerminalName::Unknown,
        };
        return TerminalInfo::new(
            name,
            None,
            None,
            Some(term),
            multiplexer,
            is_tty,
            color_depth,
        );
    }

    TerminalInfo::new(
        TerminalName::Unknown,
        None,
        None,
        None,
        multiplexer,
        is_tty,
        color_depth,
    )
}

fn detect_color_depth<E: Environment>(env: &E) -> ColorDepth {
    if let Some(ct) = env.var("COLORTERM")
        && (ct == "truecolor" || ct == "24bit")
    {
        return ColorDepth::TrueColor;
    }
    if let Some(term) = env.var("TERM") {
        if term == "dumb" {
            return ColorDepth::NoColor;
        }
        if term.contains("256color")
            || term.contains("ghostty")
            || term.contains("kitty")
            || term.contains("alacritty")
        {
            return ColorDepth::Ansi256;
        }
    }
    ColorDepth::Ansi16
}

fn detect_multiplexer<E: Environment>(env: &E) -> Option<Multiplexer> {
    if let Some(version) = env.var("ZELLIJ_VERSION") {
        return Some(Multiplexer::Zellij {
            version: if version.is_empty() {
                None
            } else {
                Some(version)
            },
        });
    }

    if env.var("ZELLIJ").is_some() {
        return Some(Multiplexer::Zellij { version: None });
    }

    if env.var("TMUX").is_some() {
        let version = env
            .var("TERM_PROGRAM")
            .filter(|p| p == "tmux")
            .and_then(|_| env.var("TERM_PROGRAM_VERSION"));
        return Some(Multiplexer::Tmux { version });
    }

    None
}

static CURRENT_TERMINAL: OnceLock<TerminalInfo> = OnceLock::new();

/// Global accessor for current terminal detection.
pub fn current_terminal() -> &'static TerminalInfo {
    CURRENT_TERMINAL.get_or_init(|| detect_terminal_from_env(&SystemEnvironment))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestEnvironment {
        vars: HashMap<String, String>,
    }

    impl Environment for TestEnvironment {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned()
        }
    }

    #[test]
    fn test_detect_ghostty() {
        let mut vars = HashMap::new();
        vars.insert("TERM_PROGRAM".to_string(), "ghostty".to_string());
        vars.insert("TERM_PROGRAM_VERSION".to_string(), "1.0.0".to_string());
        let env = TestEnvironment { vars };

        let info = detect_terminal_from_env(&env);
        assert_eq!(info.name, TerminalName::Ghostty);
        assert_eq!(info.user_agent_token(), "ghostty/1.0.0");
    }

    #[test]
    fn test_detect_vscode() {
        let mut vars = HashMap::new();
        vars.insert("TERM_PROGRAM".to_string(), "vscode".to_string());
        vars.insert("TERM_PROGRAM_VERSION".to_string(), "1.95.3".to_string());
        let env = TestEnvironment { vars };

        let info = detect_terminal_from_env(&env);
        assert_eq!(info.name, TerminalName::VsCode);
        assert_eq!(info.user_agent_token(), "vscode/1.95.3");
    }

    #[test]
    fn test_detect_kitty() {
        let mut vars = HashMap::new();
        vars.insert("KITTY_PID".to_string(), "12345".to_string());
        let env = TestEnvironment { vars };

        let info = detect_terminal_from_env(&env);
        assert_eq!(info.name, TerminalName::Kitty);
        assert_eq!(info.user_agent_token(), "kitty");
    }

    #[test]
    fn test_detect_zellij_multiplexer() {
        let mut vars = HashMap::new();
        vars.insert("ZELLIJ_VERSION".to_string(), "0.40.1".to_string());
        vars.insert("TERM".to_string(), "xterm-256color".to_string());
        let env = TestEnvironment { vars };

        let info = detect_terminal_from_env(&env);
        assert!(info.is_zellij());
        assert_eq!(
            info.multiplexer,
            Some(Multiplexer::Zellij {
                version: Some("0.40.1".to_string())
            })
        );
    }
}
