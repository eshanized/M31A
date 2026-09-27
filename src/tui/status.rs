//! 12-Status Presentation Taxonomy (TDS-04, D-03).
//!
//! Provides the canonical dual symbol/bracketed badges and styling tokens
//! for all operational statuses in M31A. Ensures color is never the sole
//! carrier of semantic information.

use ratatui::style::Style;
use serde::{Deserialize, Serialize};

use super::theme::ThemeTokens;

/// The canonical 12-status taxonomy for all tasks, missions, jobs, and tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusKind {
    Ok,
    Running,
    Waiting,
    Blocked,
    Failed,
    Warning,
    Asking,
    Denied,
    Planning,
    Verifying,
    Paused,
    Cancelled,
}

impl StatusKind {
    /// Returns the standard bracketed text badge.
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Ok => "[OK]",
            Self::Running => "[RUN]",
            Self::Waiting => "[WAIT]",
            Self::Blocked => "[BLOCK]",
            Self::Failed => "[FAIL]",
            Self::Warning => "[WARN]",
            Self::Asking => "[ASK]",
            Self::Denied => "[DENY]",
            Self::Planning => "[PLAN]",
            Self::Verifying => "[VERIFY]",
            Self::Paused => "[PAUSE]",
            Self::Cancelled => "[CANCEL]",
        }
    }

    /// Returns human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ok => "Success",
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Blocked => "Blocked",
            Self::Failed => "Failed",
            Self::Warning => "Warning",
            Self::Asking => "Input Required",
            Self::Denied => "Denied",
            Self::Planning => "Planning",
            Self::Verifying => "Verifying",
            Self::Paused => "Paused",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// Helper for rendering status badges with appropriate theme styles.
pub struct StatusPresentation;

impl StatusPresentation {
    /// Formats a status kind into a `(badge, style)` pair using active theme tokens.
    pub fn format(kind: StatusKind, theme: &ThemeTokens) -> (&'static str, Style) {
        let style = match kind {
            StatusKind::Ok => theme.status_ok,
            StatusKind::Running => theme.status_running,
            StatusKind::Waiting => theme.status_waiting,
            StatusKind::Blocked => theme.status_blocked,
            StatusKind::Failed => theme.status_failed,
            StatusKind::Warning => theme.status_warning,
            StatusKind::Asking => theme.status_asking,
            StatusKind::Denied => theme.status_denied,
            StatusKind::Planning => theme.status_planning,
            StatusKind::Verifying => theme.status_verifying,
            StatusKind::Paused => theme.status_paused,
            StatusKind::Cancelled => theme.status_cancelled,
        };

        (kind.badge(), style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::ThemeMode;

    #[test]
    fn test_all_twelve_statuses_have_unique_badges() {
        let kinds = [
            StatusKind::Ok,
            StatusKind::Running,
            StatusKind::Waiting,
            StatusKind::Blocked,
            StatusKind::Failed,
            StatusKind::Warning,
            StatusKind::Asking,
            StatusKind::Denied,
            StatusKind::Planning,
            StatusKind::Verifying,
            StatusKind::Paused,
            StatusKind::Cancelled,
        ];

        let mut badges = std::collections::HashSet::new();
        for kind in kinds {
            assert!(badges.insert(kind.badge()));
        }
        assert_eq!(badges.len(), 12);
    }

    #[test]
    fn test_status_presentation_formatting() {
        let theme = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
        let (badge, _) = StatusPresentation::format(StatusKind::Ok, &theme);
        assert_eq!(badge, "[OK]");
    }
}
