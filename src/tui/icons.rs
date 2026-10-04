//! Semantic Nerd Font Icon System with Safe Fallback (TUI-03, D-15).
//!
//! Provides a single canonical icon registry used by all TUI rendering. Never
//! scatters raw icon literals throughout the code. Supports Nerd Font mode
//! and a safe ASCII/Unicode fallback so the TUI remains readable on:
//! - Nerd Font terminal
//! - normal Unicode terminal
//! - restricted ANSI terminal

use crate::tui::theme::ThemeMode;

/// Rendering mode for the icon system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconMode {
    /// Full Nerd Font glyphs (requires a Nerd Font-patched font).
    NerdFont,
    /// Safe Unicode fallback (works on standard Unicode terminals).
    Unicode,
    /// Minimal ASCII fallback (works everywhere).
    Ascii,
}

impl IconMode {
    /// Detect the appropriate mode from environment and theme.
    pub fn detect(theme_mode: ThemeMode) -> Self {
        if std::env::var("M31A_ICONS_ASCII").is_ok() {
            return Self::Ascii;
        }
        if std::env::var("M31A_ICONS_UNICODE").is_ok() {
            return Self::Unicode;
        }
        // Default: attempt Nerd Font unless monochrome/ANSI-only theme
        match theme_mode {
            ThemeMode::MonochromeANSI => Self::Ascii,
            ThemeMode::Default
            | ThemeMode::DarkSlateCyan
            | ThemeMode::HighContrast
            | ThemeMode::CleanLight => Self::NerdFont,
        }
    }
}

/// Central semantic icon registry.
///
/// All TUI surfaces must resolve icons through this registry. Never embed
/// raw glyphs in render code.
#[derive(Debug, Clone)]
pub struct IconRegistry {
    mode: IconMode,
}

impl IconRegistry {
    /// Create a new registry for the given mode.
    pub fn new(mode: IconMode) -> Self {
        Self { mode }
    }

    /// Create a registry auto-detected from the current theme.
    pub fn from_theme(theme_mode: ThemeMode) -> Self {
        Self::new(IconMode::detect(theme_mode))
    }

    /// Get the current rendering mode.
    pub fn mode(&self) -> IconMode {
        self.mode
    }

    /// Resolve a semantic icon by key.
    pub fn get(&self, key: IconKey) -> &'static str {
        match self.mode {
            IconMode::NerdFont => key.nerd_font(),
            IconMode::Unicode => key.unicode(),
            IconMode::Ascii => key.ascii(),
        }
    }
}

/// Semantic icon keys. Extend this enum for new icons rather than adding
/// raw glyphs to render code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKey {
    // Status / state
    Success,
    Error,
    Warning,
    Info,
    // Roles
    User,
    Assistant,
    System,
    // Activity
    Thinking,
    Working,
    Idle,
    Discovering,
    Planning,
    WaitingForReview,
    Executing,
    RunningTool,
    Verifying,
    Recovering,
    WaitingForApproval,
    Completed,
    Failed,
    Cancelled,
    // Objects
    Tool,
    Terminal,
    File,
    Folder,
    Git,
    Branch,
    Model,
    Provider,
    Plan,
    Task,
    Verification,
    Security,
    Approval,
    Cancel,
    Search,
    Command,
    Clock,
    Network,
    // Governance badges
    PlanReview,
    TaskReview,
    AuthRequired,
    AuthGranted,
    // Lifecycle stages
    DiscoveryRequired,
    PlanDraft,
    PlanRevisionAvailable,
    PlanAccepted,
    TasksDraft,
    TaskRevisionAvailable,
    TasksAccepted,
    ExecutionAuthorized,
    VerifyingStage,
    CompletedStage,
    FailedStage,
    Rejected,
    CancelledStage,
    Blocked,
}

impl IconKey {
    /// Nerd Font glyph (requires Nerd Font-patched font).
    const fn nerd_font(&self) -> &'static str {
        match self {
            // Status
            Self::Success => "\u{f058}", // nf-fa-check_circle
            Self::Error => "\u{f057}",   // nf-fa-times_circle
            Self::Warning => "\u{f071}", // nf-fa-exclamation_triangle
            Self::Info => "\u{f05a}",    // nf-fa-info_circle
            // Roles
            Self::User => "\u{f007}",      // nf-fa-user
            Self::Assistant => "\u{f1b3}", // nf-fa-robot
            Self::System => "\u{f108}",    // nf-fa-cogs
            // Activity
            Self::Thinking => "\u{f24d}",           // nf-fa-thinking
            Self::Working => "\u{f110}",            // nf-fa-spinner
            Self::Idle => "\u{f23e}",               // nf-fa-circle_o
            Self::Discovering => "\u{f002}",        // nf-fa-search
            Self::Planning => "\u{f249}",           // nf-fa-file_text
            Self::WaitingForReview => "\u{f071}",   // nf-fa-exclamation_triangle
            Self::Executing => "\u{f017}",          // nf-fa-clock_o
            Self::RunningTool => "\u{f0ad}",        // nf-fa-wrench
            Self::Verifying => "\u{f058}",          // nf-fa-check_circle
            Self::Recovering => "\u{f01e}",         // nf-fa-refresh
            Self::WaitingForApproval => "\u{f0e5}", // nf-fa-gavel
            Self::Completed => "\u{f00c}",          // nf-fa-check
            Self::Failed => "\u{f00d}",             // nf-fa-times
            Self::Cancelled => "\u{f05e}",          // nf-fa-ban
            // Objects
            Self::Tool => "\u{f0ad}",         // nf-fa-wrench
            Self::Terminal => "\u{f120}",     // nf-fa-terminal
            Self::File => "\u{f15b}",         // nf-fa-file_text_o
            Self::Folder => "\u{f07c}",       // nf-fa-folder
            Self::Git => "\u{f1d3}",          // nf-fa-git
            Self::Branch => "\u{f126}",       // nf-fa-code_fork
            Self::Model => "\u{f2db}",        // nf-fa-brain
            Self::Provider => "\u{f0c3}",     // nf-fa-cloud
            Self::Plan => "\u{f0f6}",         // nf-fa-sitemap
            Self::Task => "\u{f0ae}",         // nf-fa-tasks
            Self::Verification => "\u{f058}", // nf-fa-check_circle
            Self::Security => "\u{f023}",     // nf-fa-lock
            Self::Approval => "\u{f0e5}",     // nf-fa-gavel
            Self::Cancel => "\u{f05e}",       // nf-fa-ban
            Self::Search => "\u{f002}",       // nf-fa-search
            Self::Command => "\u{f120}",      // nf-fa-terminal
            Self::Clock => "\u{f017}",        // nf-fa-clock_o
            Self::Network => "\u{f0ac}",      // nf-fa-globe
            // Governance
            Self::PlanReview => "\u{f0f6}",   // nf-fa-sitemap
            Self::TaskReview => "\u{f0ae}",   // nf-fa-tasks
            Self::AuthRequired => "\u{f023}", // nf-fa-lock
            Self::AuthGranted => "\u{f09c}",  // nf-fa-unlock
            // Lifecycle
            Self::DiscoveryRequired => "\u{f002}", // nf-fa-search
            Self::PlanDraft => "\u{f249}",         // nf-fa-file_text
            Self::PlanRevisionAvailable => "\u{f249}", // nf-fa-file_text
            Self::PlanAccepted => "\u{f00c}",      // nf-fa-check
            Self::TasksDraft => "\u{f0ae}",        // nf-fa-tasks
            Self::TaskRevisionAvailable => "\u{f0ae}", // nf-fa-tasks
            Self::TasksAccepted => "\u{f00c}",     // nf-fa-check
            Self::ExecutionAuthorized => "\u{f09c}", // nf-fa-unlock
            Self::VerifyingStage => "\u{f058}",    // nf-fa-check_circle
            Self::CompletedStage => "\u{f00c}",    // nf-fa-check
            Self::FailedStage => "\u{f00d}",       // nf-fa-times
            Self::Rejected => "\u{f05e}",          // nf-fa-ban
            Self::CancelledStage => "\u{f05e}",    // nf-fa-ban
            Self::Blocked => "\u{f05e}",           // nf-fa-ban
        }
    }

    /// Unicode fallback (works on standard Unicode terminals).
    const fn unicode(&self) -> &'static str {
        match self {
            // Status
            Self::Success => "✓",
            Self::Error => "✗",
            Self::Warning => "⚠",
            Self::Info => "ℹ",
            // Roles
            Self::User => "◈",
            Self::Assistant => "◆",
            Self::System => "⚙",
            // Activity
            Self::Thinking => "⋯",
            Self::Working => "⟳",
            Self::Idle => "○",
            Self::Discovering => "🔍",
            Self::Planning => "📋",
            Self::WaitingForReview => "⏳",
            Self::Executing => "▶",
            Self::RunningTool => "⚙",
            Self::Verifying => "✓",
            Self::Recovering => "↻",
            Self::WaitingForApproval => "🔐",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::Cancelled => "⊘",
            // Objects
            Self::Tool => "⚙",
            Self::Terminal => "□",
            Self::File => "📄",
            Self::Folder => "📁",
            Self::Git => "±",
            Self::Branch => "⌥",
            Self::Model => "🧠",
            Self::Provider => "☁",
            Self::Plan => "📋",
            Self::Task => "☐",
            Self::Verification => "✓",
            Self::Security => "🔒",
            Self::Approval => "🔐",
            Self::Cancel => "⊘",
            Self::Search => "🔍",
            Self::Command => "□",
            Self::Clock => "🕐",
            Self::Network => "🌐",
            // Governance
            Self::PlanReview => "📋",
            Self::TaskReview => "☐",
            Self::AuthRequired => "🔒",
            Self::AuthGranted => "🔓",
            // Lifecycle
            Self::DiscoveryRequired => "?",
            Self::PlanDraft => "📄",
            Self::PlanRevisionAvailable => "📄",
            Self::PlanAccepted => "✓",
            Self::TasksDraft => "☐",
            Self::TaskRevisionAvailable => "☐",
            Self::TasksAccepted => "✓",
            Self::ExecutionAuthorized => "🔓",
            Self::VerifyingStage => "✓",
            Self::CompletedStage => "✓",
            Self::FailedStage => "✗",
            Self::Rejected => "⊘",
            Self::CancelledStage => "⊘",
            Self::Blocked => "⊘",
        }
    }

    /// ASCII fallback (works everywhere, no Unicode required).
    const fn ascii(&self) -> &'static str {
        match self {
            // Status
            Self::Success => "[OK]",
            Self::Error => "[ERR]",
            Self::Warning => "[WARN]",
            Self::Info => "[INFO]",
            // Roles
            Self::User => "[USR]",
            Self::Assistant => "[BOT]",
            Self::System => "[SYS]",
            // Activity
            Self::Thinking => "[...]",
            Self::Working => "[*]",
            Self::Idle => "[ ]",
            Self::Discovering => "[?]",
            Self::Planning => "[PLAN]",
            Self::WaitingForReview => "[REV]",
            Self::Executing => "[RUN]",
            Self::RunningTool => "[TOOL]",
            Self::Verifying => "[VER]",
            Self::Recovering => "[REC]",
            Self::WaitingForApproval => "[AUTH]",
            Self::Completed => "[OK]",
            Self::Failed => "[FAIL]",
            Self::Cancelled => "[CANC]",
            // Objects
            Self::Tool => "[TOOL]",
            Self::Terminal => "[TERM]",
            Self::File => "[FILE]",
            Self::Folder => "[DIR]",
            Self::Git => "[GIT]",
            Self::Branch => "[BR]",
            Self::Model => "[MDL]",
            Self::Provider => "[PRV]",
            Self::Plan => "[PLAN]",
            Self::Task => "[TSK]",
            Self::Verification => "[VER]",
            Self::Security => "[SEC]",
            Self::Approval => "[APPR]",
            Self::Cancel => "[CANC]",
            Self::Search => "[SRCH]",
            Self::Command => "[CMD]",
            Self::Clock => "[TIME]",
            Self::Network => "[NET]",
            // Governance
            Self::PlanReview => "[PREV]",
            Self::TaskReview => "[TREV]",
            Self::AuthRequired => "[AUTH]",
            Self::AuthGranted => "[OK]",
            // Lifecycle
            Self::DiscoveryRequired => "[DISC]",
            Self::PlanDraft => "[PDFT]",
            Self::PlanRevisionAvailable => "[PREV]",
            Self::PlanAccepted => "[OK]",
            Self::TasksDraft => "[TDFT]",
            Self::TaskRevisionAvailable => "[TREV]",
            Self::TasksAccepted => "[OK]",
            Self::ExecutionAuthorized => "[AUTH]",
            Self::VerifyingStage => "[VER]",
            Self::CompletedStage => "[OK]",
            Self::FailedStage => "[FAIL]",
            Self::Rejected => "[REJ]",
            Self::CancelledStage => "[CANC]",
            Self::Blocked => "[BLK]",
        }
    }
}

/// Spinner frames for animated activity indicators.
#[derive(Debug, Clone)]
pub struct Spinner {
    frames: &'static [&'static str],
    frame: usize,
}

impl Spinner {
    /// Create a new spinner with the standard Braille frames.
    pub fn new() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            frame: 0,
        }
    }

    /// Create a spinner with Nerd Font frames.
    pub fn nerd_font() -> Self {
        Self {
            frames: &["󰪥", "󰪦", "󰪧", "󰪨", "󰪩", "󰪪", "󰪫", "󰪬", "󰪭", "󰪮"],
            frame: 0,
        }
    }

    /// Create a spinner with ASCII frames.
    pub fn ascii() -> Self {
        Self {
            frames: &["|", "/", "-", "\\"],
            frame: 0,
        }
    }

    /// Advance to the next frame and return the current glyph.
    pub fn tick(&mut self) -> &str {
        let glyph = self.frames[self.frame];
        self.frame = (self.frame + 1) % self.frames.len();
        glyph
    }

    /// Return the current frame without advancing.
    pub fn current(&self) -> &str {
        self.frames[self.frame]
    }

    /// Reset to the first frame.
    pub fn reset(&mut self) {
        self.frame = 0;
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_modes_produce_output() {
        for mode in [IconMode::NerdFont, IconMode::Unicode, IconMode::Ascii] {
            let reg = IconRegistry::new(mode);
            for key in [
                IconKey::Success,
                IconKey::Error,
                IconKey::Thinking,
                IconKey::Working,
                IconKey::User,
                IconKey::Assistant,
                IconKey::Tool,
            ] {
                let glyph = reg.get(key);
                assert!(
                    !glyph.is_empty(),
                    "icon for {key:?} in {mode:?} must not be empty"
                );
            }
        }
    }

    #[test]
    fn test_spinner_cycles() {
        let mut spinner = Spinner::new();
        let first = spinner.tick();
        assert!(!first.is_empty());
        let mut seen = std::collections::HashSet::new();
        for _ in 0..20 {
            seen.insert(spinner.tick().to_string());
        }
        assert!(seen.len() > 1, "spinner must cycle through multiple frames");
    }
}
